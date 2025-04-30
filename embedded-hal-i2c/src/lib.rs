#![no_std]
#![allow(async_fn_in_trait)]

// General review note: The variation presented here has all of the behavior specified. It is possible
// to leave more of the behavior around when things are (n)acked implementation-defined.

pub use embedded_hal::i2c::I2c as SyncI2cController;
pub use embedded_hal::i2c::{
    AddressMode, Error, ErrorKind, ErrorType, NoAcknowledgeSource, Operation, SevenBitAddress,
    TenBitAddress,
};
pub use embedded_hal_async::i2c::I2c as AsyncI2cController;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AnyAddress {
    Seven(u8),
    Ten(u16),
}

impl From<SevenBitAddress> for AnyAddress {
    fn from(value: SevenBitAddress) -> Self {
        Self::Seven(value)
    }
}

impl From<TenBitAddress> for AnyAddress {
    fn from(value: TenBitAddress) -> Self {
        Self::Ten(value)
    }
}

// Returned by `listen()`
#[must_use = "Implicitly dropping a Transaction will NAK the request"]
pub enum Transaction<R, W> {
    /// For listen, a read transaction has been started and the address byte
    /// received but not yet acknowledged. The address will be acknowledged
    /// on the call to handle_part or handle_complete on the handler. To nack
    /// the address, drop the handler.
    ///
    /// For an expected read listen, the entire buffer has been
    /// sent to the master and the master desires more bytes.
    ReadTransaction { address: AnyAddress, handler: R },
    /// For listen, a write transaction has been started and the address byte
    /// received but not yet acknowledged. The address will be acknowledged
    /// on the call to handle_part or handle_complete on the handler. To nack
    /// the address, drop the handler.
    ///
    /// For an expected write listen, the entire buffer has been
    /// read from the master and the master wants to send more bytes.
    WriteTransaction { address: AnyAddress, handler: W },
}

/// Returned by `listen_expect_read()`
#[must_use = "Implicitly dropping a Transaction will NAK the request"]
pub enum ExpectHandledRead<R, W> {
    /// A read was handled completely as expected
    HandledCompletely(usize),
    /// The expected piece was handled, the address was acked, but
    /// the device had more for us
    HandledContinuedRead { handler: R },
    /// The expected piece was not handled, either due to a mismatched
    /// address, or mismatched transaction kind
    NotHandled(Transaction<R, W>),
}

/// Returned by `listen_expect_write()`
#[must_use = "Implicitly dropping a Transaction will NAK the request"]
pub enum ExpectHandledWrite<R, W> {
    /// A write was handled completely as expected
    HandledCompletely(usize),
    /// The expected piece was handled, the address was acked, but
    /// the device wanted more from us
    HandledContinuedWrite { handler: W },
    /// The expected piece was not handled, either due to a mismatched
    /// address, or mismatched transaction kind
    NotHandled(Transaction<R, W>),
}

pub trait I2cTarget {
    type Error;
    // Review note: Different error types for read and write transactions could
    // be interesting, but would result in either an Into bound in order for the
    // listen_expect_* functions to be able to be provided.
    type Read<'a>: ReadTransaction<Error = Self::Error> + 'a
    where
        Self: 'a;
    type Write<'a>: WriteTransaction<Error = Self::Error> + 'a
    where
        Self: 'a;

    /// Listen for a new transaction to occur
    async fn listen(&mut self)
    -> Result<Transaction<Self::Read<'_>, Self::Write<'_>>, Self::Error>;

    // Review note: Below functions could provide default implementations. They
    // are provided to allow for additional hardware acceleration.

    /// Listen for a new transaction to occur, expecting a write
    async fn listen_expect_write<'a>(
        &'a mut self,
        expected_address: AnyAddress,
        write_buffer: &mut [u8],
    ) -> Result<ExpectHandledWrite<Self::Read<'a>, Self::Write<'a>>, Self::Error> {
        match self.listen().await? {
            result @ Transaction::ReadTransaction { .. } => {
                Ok(ExpectHandledWrite::NotHandled(result))
            }
            Transaction::WriteTransaction { address, handler } => {
                if address == expected_address {
                    match handler.handle_part(write_buffer).await? {
                        WriteResult::Finished(size) => {
                            Ok(ExpectHandledWrite::HandledCompletely(size))
                        }
                        WriteResult::PartialComplete(handler) => {
                            Ok(ExpectHandledWrite::HandledContinuedWrite { handler })
                        }
                    }
                } else {
                    Ok(ExpectHandledWrite::NotHandled(
                        Transaction::WriteTransaction { address, handler },
                    ))
                }
            }
        }
    }
    /// Listen for a new transaction to occur, expecting a read
    async fn listen_expect_read<'a>(
        &'a mut self,
        expected_address: AnyAddress,
        read_buffer: &[u8],
    ) -> Result<ExpectHandledRead<Self::Read<'a>, Self::Write<'a>>, Self::Error> {
        match self.listen().await? {
            result @ Transaction::WriteTransaction { .. } => {
                Ok(ExpectHandledRead::NotHandled(result))
            }
            Transaction::ReadTransaction { address, handler } if address == expected_address => {
                match handler.handle_part(read_buffer).await? {
                    ReadResult::Finished(size) => Ok(ExpectHandledRead::HandledCompletely(size)),
                    ReadResult::PartialComplete(handler) => {
                        Ok(ExpectHandledRead::HandledContinuedRead { handler })
                    }
                }
            }
            result @ Transaction::ReadTransaction { .. } => {
                Ok(ExpectHandledRead::NotHandled(result))
            }
        }
    }

    /// Listen for a new transaction to occur, expecting either
    // TODO: Add extra Transaction return type?
    async fn listen_expect_either<'a>(
        &'a mut self,
        expected_address: AnyAddress,
        read_buffer: &[u8],
        write_buffer: &mut [u8],
    ) -> Result<Transaction<Self::Read<'a>, Self::Write<'a>>, Self::Error> {
        let _ = (expected_address, read_buffer, write_buffer);
        todo!()
    }
}

/// Result of partial handling of a read transaction
#[must_use = "Implicitly dropping a Transaction will NAK the request"]
pub enum ReadResult<R> {
    Finished(usize),
    PartialComplete(R),
}

/// Handler for a read transaction
///
/// On drop, will set the hardware to provide an implementation-defined overrun character
/// for the rest of the read. If the address was not yet acknowledged, dropping will nack the address.
pub trait ReadTransaction: Sized {
    type Error;
    /// Provide part of the data for the read transaction
    async fn handle_part(self, buffer: &[u8]) -> Result<ReadResult<Self>, Self::Error>;

    /// Finish the entire read transaction, providing the overrun character once the buffer runs out
    async fn handle_complete(self, buffer: &[u8], ovc: u8) -> Result<usize, Self::Error> {
        match self.handle_part(buffer).await? {
            ReadResult::Finished(size) => Ok(size),
            ReadResult::PartialComplete(mut this) => {
                let mut total = buffer.len();
                loop {
                    match this.handle_part(&[ovc]).await? {
                        ReadResult::Finished(extra) => break Ok(total + extra),
                        ReadResult::PartialComplete(handler) => {
                            this = handler;
                            total += 1;
                        }
                    }
                }
            }
        }
    }
}

/// Result of partial handling of a write transaction
#[must_use = "Implicitly dropping a Transaction will NAK the request"]
pub enum WriteResult<W> {
    Finished(usize),
    PartialComplete(W),
}

/// Handler for a write transaction
///
/// On drop, will nack the last byte and end the transaction
pub trait WriteTransaction: Sized {
    type Error;

    /// Accept buffer.len bytes of the write, acknowledging all but the last byte. The last byte
    /// is neither acknowledged nor not acknowledged.
    async fn handle_part(self, buffer: &mut [u8]) -> Result<WriteResult<Self>, Self::Error>;

    /// Accept buffer.len bytes of the write, acknowledging all but the last byte, and nacking on the last byte.
    async fn handle_complete(self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        match self.handle_part(buffer).await? {
            WriteResult::Finished(size) => Ok(size),
            WriteResult::PartialComplete(_) => Ok(buffer.len()),
        }
    }
}
