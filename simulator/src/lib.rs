use embedded_hal::i2c::{AddressMode, ErrorKind, ErrorType, NoAcknowledgeSource, Operation};
use embedded_hal_async::i2c::I2c as AsyncI2c;
use embedded_hal_i2c::{
    I2cTarget, ReadResult, ReadTransaction, Transaction, WriteResult, WriteTransaction,
};
use std::mem::ManuallyDrop;
use tokio::sync::mpsc::{Receiver, Sender, channel};

pub fn simulator<A: AddressMode>(address: A) -> (SimController<A>, SimTarget<A>) {
    let (to_controller, from_target) = channel(2);
    let (to_target, from_controller) = channel(2);

    (
        SimController {
            to_target,
            from_target,
        },
        SimTarget {
            address,
            state: Default::default(),
            to_controller,
            from_controller,
        },
    )
}

#[derive(Debug)]
enum ControllerAction<A> {
    Start,
    Restart,
    Stop,
    Address { address: A, is_read: bool },
    WriteByte(u8),
    RequestByte,
    AckRead,
}

#[derive(Debug)]
enum TargetReaction {
    AckAddress,
    NackAddress,
    AckWrite,
    NackWrite,
    ReadByte(u8),
}

/// Makes a [I2cTarget] usable as a [embedded-hal::i2c::I2c]
pub struct SimController<A> {
    to_target: Sender<ControllerAction<A>>,
    from_target: Receiver<TargetReaction>,
}

impl<A> SimController<A> {
    async fn send(&mut self, action: ControllerAction<A>) {
        self.to_target.send(action).await.unwrap();
    }

    async fn transact(&mut self, action: ControllerAction<A>) -> TargetReaction {
        self.to_target.send(action).await.unwrap();
        self.from_target.recv().await.unwrap()
    }

    async fn send_address(
        &mut self,
        address: A,
        is_read: bool,
    ) -> Result<(), <SimController<A> as ErrorType>::Error> {
        let resp = self
            .transact(ControllerAction::Address { address, is_read })
            .await;

        match resp {
            TargetReaction::AckAddress => Ok(()),
            TargetReaction::NackAddress => {
                self.send(ControllerAction::Stop).await;
                Err(ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address))
            }
            TargetReaction::AckWrite | TargetReaction::NackWrite | TargetReaction::ReadByte(_) => {
                unreachable!()
            }
        }?;
        Ok(())
    }

    async fn handle_read(&mut self, address: A, buf: &mut [u8]) -> Result<(), ErrorKind> {
        self.send_address(address, true).await?;

        for b in buf {
            let resp = self.transact(ControllerAction::RequestByte).await;
            let TargetReaction::ReadByte(byte) = resp else {
                unreachable!()
            };
            *b = byte;
            self.send(ControllerAction::AckRead).await;
        }

        Ok(())
    }

    async fn handle_write(&mut self, address: A, buf: &[u8]) -> Result<(), ErrorKind> {
        self.send_address(address, false).await?;

        for b in buf {
            let resp = self.transact(ControllerAction::WriteByte(*b)).await;
            match resp {
                TargetReaction::AckWrite => continue,
                TargetReaction::NackWrite => {
                    self.send(ControllerAction::Stop).await;
                    return Err(ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data));
                }
                TargetReaction::AckAddress
                | TargetReaction::NackAddress
                | TargetReaction::ReadByte(_) => {
                    unreachable!()
                }
            }
        }

        Ok(())
    }
}

impl<A> ErrorType for SimController<A> {
    type Error = ErrorKind;
}

impl<A: AddressMode + Copy> AsyncI2c<A> for SimController<A> {
    async fn transaction(
        &mut self,
        address: A,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        let mut first = true;
        for op in operations {
            if first {
                first = false;
                self.send(ControllerAction::Start).await;
            } else {
                self.send(ControllerAction::Restart).await;
            }

            match op {
                Operation::Read(buf) => {
                    self.handle_read(address, buf).await?;
                }
                Operation::Write(buf) => {
                    self.handle_write(address, buf).await?;
                }
            }
        }
        self.send(ControllerAction::Stop).await;

        Ok(())
    }
}

pub struct SimTarget<A> {
    address: A,
    state: TargetState,
    to_controller: Sender<TargetReaction>,
    from_controller: Receiver<ControllerAction<A>>,
}

impl<A: core::fmt::Debug> SimTarget<A> {
    async fn react(&mut self, action: TargetReaction) {
        println!("C<T: {action:?}");
        self.to_controller.send(action).await.unwrap();
    }

    async fn recv(&mut self) -> ControllerAction<A> {
        let val = self.from_controller.recv().await.unwrap();
        println!("C>T: {val:?}");
        val
    }
}

impl<A: AddressMode + PartialEq + core::fmt::Debug> I2cTarget<A> for SimTarget<A> {
    type Error = ErrorKind;
    type Read<'a> = OnRead<'a, A>;
    type Write<'a> = OnWrite<'a, A>;

    async fn listen(
        &mut self,
    ) -> Result<Transaction<A, Self::Read<'_>, Self::Write<'_>>, Self::Error> {
        loop {
            match (self.state.clone(), self.recv().await) {
                (TargetState::WaitForStart, ControllerAction::Start) => {
                    self.state = TargetState::WaitForAddress;
                    continue;
                }
                (TargetState::WaitForAddress, ControllerAction::Address { address, .. })
                    if address != self.address =>
                {
                    self.state = TargetState::WaitForStop;
                    self.react(TargetReaction::NackAddress).await;
                    continue;
                }
                (TargetState::WaitForAddress, ControllerAction::Address { address, is_read }) => {
                    self.state = TargetState::InTransaction;
                    return Ok(if is_read {
                        Transaction::ReadTransaction {
                            address,
                            handler: OnRead::new(self),
                        }
                    } else {
                        Transaction::WriteTransaction {
                            address,
                            handler: OnWrite::new(self),
                        }
                    });
                }
                (
                    TargetState::InTransaction | TargetState::WaitForStop,
                    ControllerAction::Restart,
                ) => {
                    self.state = TargetState::WaitForAddress;
                }
                (TargetState::InTransaction | TargetState::WaitForStop, ControllerAction::Stop) => {
                    self.state = TargetState::WaitForStart;
                }
                unexpected => {
                    panic!("Unexpected ControllerAction for State: {:?}", unexpected)
                }
            }
        }
    }
}

pub struct OnRead<'a, A: core::fmt::Debug> {
    inner: &'a mut SimTarget<A>,
    ack_sent: bool,
}

impl<'a, A: core::fmt::Debug> OnRead<'a, A> {
    const FILL: u8 = 0x2a;

    const fn new(inner: &'a mut SimTarget<A>) -> Self {
        Self {
            inner,
            ack_sent: false,
        }
    }

    async fn send_byte(&mut self, byte: u8) -> Result<(), ()> {
        if !self.ack_sent {
            self.inner.react(TargetReaction::AckAddress).await;
            self.ack_sent = true;
        }

        match self.inner.recv().await {
            ControllerAction::RequestByte => {}
            ControllerAction::Restart => {
                self.inner.state = TargetState::WaitForAddress;
                return Err(());
            }
            ControllerAction::Stop => {
                self.inner.state = TargetState::WaitForStart;
                return Err(());
            }
            unexpected => {
                panic!(
                    "Controller sent unexpected {unexpected:?} in response to a read instead of an ACK/NACK"
                )
            }
        }

        self.inner.react(TargetReaction::ReadByte(byte)).await;

        match self.inner.recv().await {
            ControllerAction::AckRead => Ok(()),
            ControllerAction::Restart => {
                self.inner.state = TargetState::WaitForAddress;
                Err(())
            }
            ControllerAction::Stop => {
                self.inner.state = TargetState::WaitForStart;
                Err(())
            }
            unexpected => {
                panic!(
                    "Controller sent unexpected {unexpected:?} in response to a read instead of an ACK/NACK"
                )
            }
        }
    }

    const fn defuse(self) {
        let _ = ManuallyDrop::new(self);
    }

    async fn handle_rest(&mut self) {
        if !self.ack_sent {
            self.inner.react(TargetReaction::NackAddress).await;
            self.inner.state = TargetState::WaitForStop;
        } else {
            while let Ok(()) = self.send_byte(Self::FILL).await {}
        }
    }
}

impl<A: core::fmt::Debug> Drop for OnRead<'_, A> {
    fn drop(&mut self) {
        panic!("Do not drop Read handles!");
    }
}

impl<A: core::fmt::Debug> ReadTransaction for OnRead<'_, A> {
    type Error = ErrorKind;

    async fn handle_part(mut self, buffer: &[u8]) -> Result<ReadResult<Self>, Self::Error> {
        for (idx, b) in buffer.iter().enumerate() {
            match self.send_byte(*b).await {
                Ok(()) => continue,
                Err(()) => {
                    // Avoid sending more data in `drop()`
                    self.defuse();

                    // TODO check for of-by-one
                    return Ok(ReadResult::Finished(idx));
                }
            }
        }

        Ok(ReadResult::PartialComplete(self))
    }

    async fn done(mut self) {
        self.handle_rest().await;

        // Inhibit drop calling defuse_inner again
        self.defuse();
    }
}

pub struct OnWrite<'a, A> {
    inner: &'a mut SimTarget<A>,
    ack_sent: bool,
}

impl<'a, A: core::fmt::Debug> OnWrite<'a, A> {
    const fn new(inner: &'a mut SimTarget<A>) -> Self {
        Self {
            inner,
            ack_sent: false,
        }
    }

    async fn recv_byte(&mut self) -> Result<u8, ()> {
        if !self.ack_sent {
            self.inner.react(TargetReaction::AckAddress).await;
            self.ack_sent = true;
        }

        match self.inner.recv().await {
            ControllerAction::WriteByte(byte) => {
                self.inner.react(TargetReaction::AckWrite).await;
                Ok(byte)
            }
            ControllerAction::Restart => {
                self.inner.state = TargetState::WaitForAddress;
                Err(())
            }
            ControllerAction::Stop => {
                self.inner.state = TargetState::WaitForStart;
                Err(())
            }
            action @ (ControllerAction::Start
            | ControllerAction::Address { .. }
            | ControllerAction::RequestByte
            | ControllerAction::AckRead) => {
                panic!("Illegal controller action during write operation: {action:?}")
            }
        }
    }

    const fn defuse(self) {
        let _ = ManuallyDrop::new(self);
    }

    async fn done_inner(&mut self) {
        if !self.ack_sent {
            self.inner.react(TargetReaction::NackAddress).await;
            self.inner.state = TargetState::WaitForStop;
        } else {
            match self.inner.recv().await {
                ControllerAction::WriteByte(_) => {
                    self.inner.react(TargetReaction::NackWrite).await;
                    self.inner.state = TargetState::WaitForStop;
                }
                ControllerAction::Restart => {
                    self.inner.state = TargetState::WaitForAddress;
                }
                ControllerAction::Stop => {
                    self.inner.state = TargetState::WaitForStart;
                }
                action @ (ControllerAction::Start
                | ControllerAction::Address { .. }
                | ControllerAction::RequestByte
                | ControllerAction::AckRead) => {
                    panic!("Illegal controller action during after-write nacking: {action:?}")
                }
            }
        }
    }
}

impl<A> Drop for OnWrite<'_, A> {
    fn drop(&mut self) {
        panic!("Do not drop Write handles!");
    }
}

impl<A: core::fmt::Debug> WriteTransaction for OnWrite<'_, A> {
    type Error = ErrorKind;

    async fn handle_part(mut self, buffer: &mut [u8]) -> Result<WriteResult<Self>, Self::Error> {
        let mut filled = 0;
        for b in buffer {
            match self.recv_byte().await {
                Ok(byte) => {
                    *b = byte;
                    filled += 1;
                }
                Err(()) => {
                    self.defuse();
                    return Ok(WriteResult::Finished(filled));
                }
            }
        }

        Ok(WriteResult::PartialComplete(self))
    }

    async fn done(mut self) {
        self.done_inner().await;
        self.defuse();
    }
}

#[derive(Debug, Default, Clone)]
enum TargetState {
    #[default]
    WaitForStart,
    WaitForAddress,
    WaitForStop,
    InTransaction,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_read() {
        let (mut c, mut t) = simulator(0x42_u8);

        let control = async move {
            let mut response = [0; 8];
            c.write_read(0x42, &[1, 2, 3, 4], &mut response)
                .await
                .unwrap();

            assert_eq!(response, [1, 2, 3, 4, 5, 6, 7, 8]);
        };

        let target = async move {
            let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };

            assert_eq!(address, 0x42);
            let mut buffer = [0; 4];
            let written = handler.handle_complete(&mut buffer).await.unwrap();
            assert_eq!(written, 4);
            assert_eq!(buffer, [1, 2, 3, 4]);

            let Transaction::ReadTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(address, 0x42);
            let buffer = [1, 2, 3, 4, 5, 6, 7, 8];
            handler.handle_complete(&buffer, 0xFF).await.unwrap();
        };

        tokio::join!(control, target);
    }

    #[tokio::test]
    async fn nacking_everything() {
        let (mut c, mut t) = simulator(0x42_u8);

        let control = async move {
            let result = c.read(0x42, &mut []).await.unwrap_err();
            assert_eq!(
                result,
                ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
            );

            let result = c.write(0x42, &[]).await.unwrap_err();
            assert_eq!(
                result,
                ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
            );

            let result = c.write(0x42, &[1, 2, 3]).await.unwrap_err();
            assert_eq!(result, ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data));
        };

        let target = async move {
            let Transaction::ReadTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(address, 0x42);
            handler.done().await;

            let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(address, 0x42);
            handler.done().await;

            let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(address, 0x42);
            handler.handle_complete(&mut [0]).await.unwrap();

            // Only drop once we are done
            t
        };

        tokio::join!(control, target);
    }

    #[tokio::test]
    async fn long_transation() {
        let (mut c, mut t) = simulator(0x42_u8);

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

            c.transaction(0x42, &mut transactions).await.unwrap();

            assert_eq!(a, [3]);
            assert_eq!(b, [4]);
        };

        let target = async move {
            for expect in [1, 2] {
                let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
                else {
                    panic!()
                };
                assert_eq!(address, 0x42);
                let mut buf = [0];
                let len = handler.handle_complete(&mut buf).await.unwrap();
                assert_eq!(&buf[..len], [expect]);
            }

            for expect in [3, 4] {
                let Transaction::ReadTransaction { address, handler } = t.listen().await.unwrap()
                else {
                    panic!()
                };
                assert_eq!(address, 0x42);
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
                assert_eq!(address, 0x42);
                let mut buf = [0];
                let len = handler.handle_complete(&mut buf).await.unwrap();
                assert_eq!(&buf[..len], [expect]);
            }
        };

        tokio::join!(control, target);
    }
}
