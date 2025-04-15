#![allow(async_fn_in_trait)]

// General review note: The variation presented here has all of the behavior specified. It is possible
// to leave more of the behavior around when things are (n)acked implementation-defined.

use embedded_hal::i2c::{AddressMode, SevenBitAddress};

// Returned by `listen()`
pub enum Transaction<A, R, W> {
    /// For listen, a read transaction has been started and the address byte
    /// received but not yet acknowledged. The address will be acknowledged
    /// on the call to handle_part or handle_complete on the handler. To nack
    /// the address, drop the handler.
    ///
    /// For an expected read listen, the entire buffer has been
    /// sent to the master and the master desires more bytes.
    ReadTransaction { address: A, handler: R },
    /// For listen, a write transaction has been started and the address byte
    /// received but not yet acknowledged. The address will be acknowledged
    /// on the call to handle_part or handle_complete on the handler. To nack
    /// the address, drop the handler.
    ///
    /// For an expected write listen, the entire buffer has been
    /// read from the master and the master wants to send more bytes.
    WriteTransaction { address: A, handler: W },
}

// Returned by `listen_expect_read()`
pub enum ExpectHandledRead<A, R, W> {
    // A read was handled completely as expected
    HandledCompletely(usize),
    // The expected piece was handled, the address was acked, but
    // the device had more for us
    HandledContinuedRead { handler: R },
    // The expected piece was not handled, either due to a mismatched
    // address, or mismatched transaction kind
    NotHandled(Transaction<A, R, W>),
}

// Returned by `listen_expect_write()`
pub enum ExpectHandledWrite<A, R, W> {
    // A write was handled completely as expected
    HandledCompletely(usize),
    // The expected piece was handled, the address was acked, but
    // the device wanted more from us
    HandledContinuedWrite { handler: W },
    // The expected piece was not handled, either due to a mismatched
    // address, or mismatched transaction kind
    NotHandled(Transaction<A, R, W>),
}

pub trait I2cTarget<A: AddressMode + PartialEq = SevenBitAddress> {
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
    async fn listen(
        &mut self,
    ) -> Result<Transaction<A, Self::Read<'_>, Self::Write<'_>>, Self::Error>;

    // Review note: Below functions could provide default implementations. They
    // are provided to allow for additional hardware acceleration.

    /// Listen for a new transaction to occur, expecting a write
    async fn listen_expect_write<'a>(
        &'a mut self,
        expected_address: A,
        write_buffer: &mut [u8],
    ) -> Result<ExpectHandledWrite<A, Self::Read<'a>, Self::Write<'a>>, Self::Error> {
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
        expected_address: A,
        read_buffer: &[u8],
    ) -> Result<ExpectHandledRead<A, Self::Read<'a>, Self::Write<'a>>, Self::Error> {
        todo!()
    }

    /// Listen for a new transaction to occur, expecting either
    // TODO: Add extra Transaction return type?
    async fn listen_expect_either<'a>(
        &'a mut self,
        expected_address: A,
        read_buffer: &[u8],
        write_buffer: &mut [u8],
    ) -> Result<Transaction<A, Self::Read<'a>, Self::Write<'a>>, Self::Error>;
}

/// Result of partial handling of a read transaction
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
            WriteResult::PartialComplete(handler) => {
                drop(handler); // sends the nack
                Ok(buffer.len())
            }
        }
    }
}
