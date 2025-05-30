//! Implementation of the target half of the simulator

use crate::{PartialTransaction, SimOp};
use embedded_hal_i2c::{
    AsyncI2cTarget, AsyncReadTransaction, AsyncWriteTransaction, ErrorKind, NoAcknowledgeSource,
    ReadResult, Transaction, WriteResult,
};
use std::cmp::min;
use tokio::sync::mpsc::Receiver;

/// Simulated I2C target
///
/// This can be created with [`crate::simulator`], which also returns the linked [`SimController`](crate::controller::SimController).
/// All [`AsyncI2cTarget::listen`], [`AsyncReadTransaction::handle_part`],
/// and [`AsyncWriteTransaction::handle_part`] calls on this target are forwarded
/// to back to the controller as if there was a real I2C bus connecting the two.
pub struct SimTarget {
    current_transaction: Option<PartialTransaction>,
    from_controller: Receiver<PartialTransaction>,
    need_to_report_deselect: bool,
}

impl SimTarget {
    pub(crate) const fn new(from_controller: Receiver<PartialTransaction>) -> Self {
        Self {
            current_transaction: None,
            from_controller,
            need_to_report_deselect: false,
        }
    }

    fn nak(&mut self, src: NoAcknowledgeSource) {
        let t = self
            .current_transaction
            .take()
            .expect("Can only be done with error if there is a transaction");

        println!("NAK transaction: {src:?}");
        assert!(!self.need_to_report_deselect);
        self.need_to_report_deselect = true;

        let _ = t.responder.send(Err(ErrorKind::NoAcknowledge(src)));
    }

    fn next(&mut self) {
        let inner = self
            .current_transaction
            .as_mut()
            .expect("Can only be done with error if there is a transaction");
        inner.current_op += 1;
    }
}

impl AsyncI2cTarget for SimTarget {
    type Error = ErrorKind;
    type Read<'a> = OnRead<'a>;
    type Write<'a> = OnWrite<'a>;

    async fn listen(
        &mut self,
    ) -> Result<Transaction<Self::Read<'_>, Self::Write<'_>>, Self::Error> {
        if self.need_to_report_deselect {
            self.need_to_report_deselect = false;
            return Ok(Transaction::Deselect);
        }

        let current = match &mut self.current_transaction {
            Some(current) => current,
            None => {
                let new = self.from_controller.recv().await.ok_or(ErrorKind::Other)?;
                println!("New transaction: {:?}", new.transaction);
                self.current_transaction.insert(new)
            }
        };

        let address = current.transaction.address;

        Ok(match current.current_mut() {
            None => {
                // We are done with this one wait for the next
                let done = self.current_transaction.take().unwrap();
                assert_eq!(done.current_op, done.transaction.actions.len());
                println!("ACK transaction: {:?}", done.transaction);
                let _ = done.responder.send(Ok(done.transaction));
                Transaction::Deselect
            }
            Some(SimOp::Read(_)) => Transaction::Read {
                address,
                handler: OnRead::new(self),
            },
            Some(SimOp::Write(_)) => Transaction::Write {
                address,
                handler: OnWrite::new(self),
            },
        })
    }
}

/// Read transaction handler for [`SimTarget`]
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

    fn current_op_mut(&mut self) -> &mut SimOp {
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
            SimOp::Read(buf) => buf,
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

impl AsyncReadTransaction for OnRead<'_> {
    type Error = ErrorKind;

    async fn handle_part(mut self, buffer: &[u8]) -> Result<ReadResult<Self>, Self::Error> {
        if buffer.is_empty() {
            // do nothing
            return Ok(ReadResult::Partial(self));
        }
        self.did_start = true;
        let target = self.remaining();

        let len = min(target.len(), buffer.len());
        target[..len].copy_from_slice(&buffer[..len]);
        self.bytes_filled += len;

        if self.remaining().is_empty() {
            Ok(ReadResult::Complete(len))
        } else {
            Ok(ReadResult::Partial(self))
        }
    }
}

/// Write transaction handler for [`SimTarget`]
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

    fn current_op(&self) -> &SimOp {
        self.inner
            .current_transaction
            .as_ref()
            .and_then(PartialTransaction::current)
            .expect("If we are in OnWrite we must have a transaction ongoing")
    }

    fn remaining(&self) -> &[u8] {
        let op = self.current_op();

        let buf = match op {
            SimOp::Write(buf) => buf,
            unexpected => panic!("Got a {unexpected:?} in OnWrite"),
        };

        &buf[self.bytes_read..]
    }

    fn disarm(self) {
        core::mem::forget(self);
    }
}

impl Drop for OnWrite<'_> {
    fn drop(&mut self) {
        if !self.did_start {
            self.inner.nak(NoAcknowledgeSource::Address);
        } else {
            self.inner.nak(NoAcknowledgeSource::Data);
        }
    }
}

impl AsyncWriteTransaction for OnWrite<'_> {
    type Error = ErrorKind;

    async fn handle_part(mut self, buffer: &mut [u8]) -> Result<WriteResult<Self>, Self::Error> {
        if buffer.is_empty() {
            // do nothing
            return Ok(WriteResult::Partial(self));
        }
        self.did_start = true;
        let source = self.remaining();

        let len = min(source.len(), buffer.len());
        buffer[..len].copy_from_slice(&source[..len]);
        self.bytes_read += len;

        if self.remaining().is_empty() {
            if buffer.len() == len {
                Ok(WriteResult::Partial(self))
            } else {
                self.inner.next();
                self.disarm();
                Ok(WriteResult::Complete(len))
            }
        } else {
            Ok(WriteResult::Partial(self))
        }
    }
}
