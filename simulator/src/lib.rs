use embedded_hal_i2c::{
    AddressMode, AnyAddress, AsyncI2cController, ErrorKind, ErrorType, I2cTarget,
    NoAcknowledgeSource, Operation, ReadResult, ReadTransaction, Transaction, WriteResult,
    WriteTransaction,
};
use std::cmp::min;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::sync::oneshot;

pub fn simulator(address: AnyAddress) -> (SimController, SimTarget) {
    let (to_target, from_controller) = channel(1);

    (
        SimController { to_target },
        SimTarget {
            address,
            current_transaction: None,
            from_controller,
        },
    )
}

#[derive(Debug, PartialEq, Eq)]
enum FooOperation {
    Read(Vec<u8>),
    Write(Vec<u8>),
}

struct FooTransaction {
    address: AnyAddress,
    actions: Vec<FooOperation>,
}

/// Makes a [I2cTarget] usable as a [embedded-hal::i2c::I2c]
pub struct SimController {
    to_target: Sender<(
        FooTransaction,
        oneshot::Sender<Result<FooTransaction, ErrorKind>>,
    )>,
}

impl ErrorType for SimController {
    type Error = ErrorKind;
}

impl<A> AsyncI2cController<A> for SimController
where
    A: AddressMode + Into<AnyAddress>,
{
    async fn transaction(
        &mut self,
        address: A,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        let address = address.into();
        let actions = operations
            .iter()
            .map(|a| match a {
                Operation::Read(r) => FooOperation::Read(vec![0; r.len()]),
                Operation::Write(w) => FooOperation::Write(w.to_vec()),
            })
            .collect();

        let transaction = FooTransaction { address, actions };
        let (sender, receiver) = oneshot::channel();

        self.to_target.try_send((transaction, sender)).unwrap();

        let response = receiver.await.map_err(|_| ErrorKind::Other)?;
        let actions = response?.actions;
        for (op, reply) in operations.iter_mut().zip(actions) {
            match (op, reply) {
                (Operation::Read(buf), FooOperation::Read(response)) => {
                    assert_eq!(buf.len(), response.len());
                    buf.copy_from_slice(&response[..]);
                }
                (Operation::Write(_), FooOperation::Write(_)) => {}
                _ => panic!("send operation does not matched received operation"),
            }
        }

        Ok(())
    }
}

struct PartialTransaction {
    transaction: FooTransaction,
    current_op: usize,
    responder: oneshot::Sender<Result<FooTransaction, ErrorKind>>,
}

impl
    From<(
        FooTransaction,
        oneshot::Sender<Result<FooTransaction, ErrorKind>>,
    )> for PartialTransaction
{
    fn from(
        value: (
            FooTransaction,
            oneshot::Sender<Result<FooTransaction, ErrorKind>>,
        ),
    ) -> Self {
        Self {
            transaction: value.0,
            current_op: 0,
            responder: value.1,
        }
    }
}

impl PartialTransaction {
    fn current(&self) -> Option<&FooOperation> {
        self.transaction.actions.get(self.current_op)
    }
    fn current_mut(&mut self) -> Option<&mut FooOperation> {
        self.transaction.actions.get_mut(self.current_op)
    }
}

pub struct SimTarget {
    address: AnyAddress,
    current_transaction: Option<PartialTransaction>,
    from_controller: Receiver<(
        FooTransaction,
        oneshot::Sender<Result<FooTransaction, ErrorKind>>,
    )>,
}

impl SimTarget {
    fn nak(&mut self, src: NoAcknowledgeSource) {
        let t = self
            .current_transaction
            .take()
            .expect("Can only be done with error if there is a transaction");

        let _ = t.responder.send(Err(ErrorKind::NoAcknowledge(src)));
    }

    fn next(&mut self) {
        let inner = self
            .current_transaction
            .as_mut()
            .expect("Can only be done with error if there is a transaction");
        inner.current_op += 1;

        if inner.current_op == inner.transaction.actions.len() {
            let me = self.current_transaction.take().unwrap();
            let _ = me.responder.send(Ok(me.transaction));
        }
    }
}

impl I2cTarget for SimTarget {
    type Error = ErrorKind;
    type Read<'a> = OnRead<'a>;
    type Write<'a> = OnWrite<'a>;

    async fn listen(
        &mut self,
    ) -> Result<Transaction<Self::Read<'_>, Self::Write<'_>>, Self::Error> {
        loop {
            let current = match &mut self.current_transaction {
                Some(current) => current,
                None => {
                    let new = self.from_controller.recv().await.ok_or(ErrorKind::Other)?;
                    let partial: PartialTransaction = new.into();

                    if self.address != partial.transaction.address {
                        let error = ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address);
                        let _ = partial.responder.send(Err(error));
                        continue;
                    }

                    self.current_transaction.insert(partial)
                }
            };

            let address = current.transaction.address;

            match current.current_mut() {
                None => {
                    // We are done with this one wait for the next
                    let self1 = self.current_transaction.take().unwrap();
                    assert_eq!(self1.current_op, self1.transaction.actions.len());
                    let _ = self1.responder.send(Ok(self1.transaction));
                    continue;
                }
                Some(FooOperation::Read(_)) => {
                    return Ok(Transaction::ReadTransaction {
                        address,
                        handler: OnRead::new(self),
                    });
                }
                Some(FooOperation::Write(_)) => {
                    return Ok(Transaction::WriteTransaction {
                        address,
                        handler: OnWrite::new(self),
                    });
                }
            }
        }
    }
}

pub struct OnRead<'a> {
    inner: &'a mut SimTarget,
    bytes_filled: usize,
    did_start: bool,
}

impl<'a> OnRead<'a> {
    const FILL: u8 = 0x2a;

    const fn new(inner: &'a mut SimTarget) -> Self {
        Self {
            inner,
            bytes_filled: 0,
            did_start: false,
        }
    }

    fn current_op_mut(&mut self) -> &mut FooOperation {
        self.inner
            .current_transaction
            .as_mut()
            .and_then(PartialTransaction::current_mut)
            .expect("If we are in OnRead we must have a transaction ongoing")
    }

    fn remaining(&mut self) -> &mut [u8] {
        let bytes_filled = self.bytes_filled;
        let op = self.current_op_mut();

        let buf = match op {
            FooOperation::Read(buf) => buf,
            unexpected => panic!("Got a {unexpected:?} in OnRead"),
        };

        &mut buf[bytes_filled..]
    }
}

impl Drop for OnRead<'_> {
    fn drop(&mut self) {
        if !self.did_start {
            self.inner.nak(NoAcknowledgeSource::Address);
        } else {
            self.remaining().fill(Self::FILL);
            self.inner.next()
        }
    }
}

impl ReadTransaction for OnRead<'_> {
    type Error = ErrorKind;

    async fn handle_part(mut self, buffer: &[u8]) -> Result<ReadResult<Self>, Self::Error> {
        self.did_start = true;
        let target = self.remaining();

        let len = min(target.len(), buffer.len());
        target[..len].copy_from_slice(&buffer[..len]);
        self.bytes_filled += len;

        if self.remaining().is_empty() {
            Ok(ReadResult::Finished(len))
        } else {
            Ok(ReadResult::PartialComplete(self))
        }
    }
}

pub struct OnWrite<'a> {
    inner: &'a mut SimTarget,
    bytes_read: usize,
    did_start: bool,
}

impl<'a> OnWrite<'a> {
    const fn new(inner: &'a mut SimTarget) -> Self {
        Self {
            inner,
            bytes_read: 0,
            did_start: false,
        }
    }

    fn current_op(&self) -> &FooOperation {
        self.inner
            .current_transaction
            .as_ref()
            .and_then(PartialTransaction::current)
            .expect("If we are in OnWrite we must have a transaction ongoing")
    }

    fn remaining(&self) -> &[u8] {
        let op = self.current_op();

        let buf = match op {
            FooOperation::Write(buf) => buf,
            unexpected => panic!("Got a {unexpected:?} in OnWrite"),
        };

        &buf[self.bytes_read..]
    }
}

impl Drop for OnWrite<'_> {
    fn drop(&mut self) {
        if !self.did_start {
            self.inner.nak(NoAcknowledgeSource::Address);
        } else if !self.remaining().is_empty() {
            self.inner.nak(NoAcknowledgeSource::Data);
        } else {
            self.inner.next()
        }
    }
}

impl WriteTransaction for OnWrite<'_> {
    type Error = ErrorKind;

    async fn handle_part(mut self, buffer: &mut [u8]) -> Result<WriteResult<Self>, Self::Error> {
        self.did_start = true;
        let source = self.remaining();

        let len = min(source.len(), buffer.len());
        buffer[..len].copy_from_slice(&source[..len]);
        self.bytes_read += len;

        if self.remaining().is_empty() {
            Ok(WriteResult::Finished(len))
        } else {
            Ok(WriteResult::PartialComplete(self))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A7: u8 = 0x42;
    const ADDR: AnyAddress = AnyAddress::Seven(A7);

    #[tokio::test]
    async fn write_read() {
        let (mut c, mut t) = simulator(ADDR);

        let control = async move {
            let mut response = [0; 8];
            c.write_read(A7, &[1, 2, 3, 4], &mut response)
                .await
                .unwrap();

            assert_eq!(response, [1, 2, 3, 4, 5, 6, 7, 8]);
        };

        let target = async move {
            let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };

            assert_eq!(address, ADDR);
            let mut buffer = [0; 4];
            let written = handler.handle_complete(&mut buffer).await.unwrap();
            assert_eq!(written, 4);
            assert_eq!(buffer, [1, 2, 3, 4]);

            let Transaction::ReadTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(address, ADDR);
            let buffer = [1, 2, 3, 4, 5, 6, 7, 8];
            handler.handle_complete(&buffer, 0xFF).await.unwrap();
        };

        tokio::join!(control, target);
    }

    #[tokio::test]
    async fn nacking_everything() {
        let (mut c, mut t) = simulator(ADDR);

        let control = async move {
            let result = c.read(A7, &mut []).await.unwrap_err();
            assert_eq!(
                result,
                ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
            );

            let result = c.write(A7, &[]).await.unwrap_err();
            assert_eq!(
                result,
                ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
            );

            let result = c.write(A7, &[1, 2, 3]).await.unwrap_err();
            assert_eq!(result, ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data));
        };

        let target = async move {
            let Transaction::ReadTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(address, ADDR);
            drop(handler);

            let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(address, ADDR);
            drop(handler);

            let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(address, ADDR);
            handler.handle_complete(&mut [0]).await.unwrap();

            // Only drop once we are done
            t
        };

        tokio::join!(control, target);
    }

    #[tokio::test]
    async fn long_transation() {
        let (mut c, mut t) = simulator(ADDR);

        let control = async move {
            let mut a = [0];
            let mut b = [0];
            let mut transactions = [
                Operation::Write(&[1]),
                Operation::Write(&[2]),
                Operation::Read(&mut a),
                Operation::Read(&mut b),
                Operation::Write(&[5]),
                Operation::Write(&[6]),
            ];

            c.transaction(A7, &mut transactions).await.unwrap();

            assert_eq!(a, [3]);
            assert_eq!(b, [4]);
        };

        let target = async move {
            for expect in [1, 2] {
                let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
                else {
                    panic!()
                };
                assert_eq!(address, ADDR);
                let mut buf = [0];
                let len = handler.handle_complete(&mut buf).await.unwrap();
                assert_eq!(&buf[..len], [expect]);
            }

            for expect in [3, 4] {
                let Transaction::ReadTransaction { address, handler } = t.listen().await.unwrap()
                else {
                    panic!()
                };
                assert_eq!(address, ADDR);
                let ReadResult::Finished(len) = handler.handle_part(&[expect, 0]).await.unwrap()
                else {
                    panic!()
                };
                assert_eq!(len, 1);
            }

            for expect in [5, 6] {
                let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
                else {
                    panic!()
                };
                assert_eq!(address, ADDR);
                let mut buf = [0];
                let len = handler.handle_complete(&mut buf).await.unwrap();
                assert_eq!(&buf[..len], [expect]);
            }
        };

        tokio::join!(control, target);
    }
}
