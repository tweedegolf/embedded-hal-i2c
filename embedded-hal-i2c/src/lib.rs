#![no_std]
#![allow(async_fn_in_trait)]

pub use embedded_hal::i2c::I2c as SyncI2cController;
pub use embedded_hal::i2c::{
    AddressMode, Error, ErrorKind, ErrorType, NoAcknowledgeSource, Operation, SevenBitAddress,
    TenBitAddress,
};
pub use embedded_hal_async::i2c::I2c as AsyncI2cController;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// An I2C slave address that is either a 7 bit or a ten bit address.
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

/// Transaction received from [`SyncI2cTarget::listen`] and
/// [`AsyncI2cTarget::listen`]
#[must_use = "Implicitly dropping a Transaction will NAK the request"]
pub enum Transaction<R, W> {
    /// A stop or restart with different address happened since the last
    /// transaction. This may be emitted multiple times between transactions.
    Deselect,
    /// A read transaction has been started and the address byte received but
    /// not yet acknowledged. The address will be acknowledged on the call to
    /// handle_part or handle_complete on the handler. To nack the address,
    /// drop the handler.
    Read {
        /// Address for which the read was received
        address: AnyAddress,
        /// Handler to be used in handling the transaction
        ///
        /// Dropping this handler nacks the address. Any other interaction
        /// acknowledges the address.
        handler: R,
    },
    /// A write transaction has been started and the address byte received but
    /// not yet acknowledged. The address will be acknowledged on the call to
    /// handle_part or handle_complete on the handler. To nack the address,
    /// drop the handler.
    Write {
        /// Address for which the write was received
        address: AnyAddress,
        /// Handler to be used in handling the transaction
        ///
        /// Dropping this handler nacks the address. Any other interaction
        /// acknowledges the address.
        handler: W,
    },
}

/// Transaction received from [`SyncI2cTarget::listen_expect_read`] and
/// [`AsyncI2cTarget::listen_expect_read`]
#[must_use = "Implicitly dropping a Transaction will NAK the request"]
pub enum TransactionExpectRead<R, W> {
    /// A read transaction was received for the expected address, and the
    /// entire transaction could be handled using the bytes provided.
    ExpectedCompleteRead { size: usize },
    /// A read transaction was received for the expected address, but more
    /// bytes are needed to complete the transaction.
    ExpectedPartialRead { handler: R },
    /// A stop or restart with different address happened since the last
    /// transaction. This may be emitted multiple times between transactions.
    Deselect,
    /// A read transaction has been started for a different address than
    /// expected. The address byte is received but not yet acknowledged. The
    /// address will be acknowledged on the call to handle_part or
    /// handle_complete on the handler. To nack the address, drop the handler.
    Read {
        /// Address for which the read was received
        address: AnyAddress,
        /// Handler to be used in handling the transaction
        ///
        /// Dropping this handler nacks the address. Any other interaction
        /// acknowledges the address.
        handler: R,
    },
    /// A write transaction has been started and the address byte received but
    /// not yet acknowledged. The address will be acknowledged on the call to
    /// handle_part or handle_complete on the handler. To nack the address,
    /// drop the handler.
    Write {
        /// Address for which the write was received
        address: AnyAddress,
        /// Handler to be used in handling the transaction
        ///
        /// Dropping this handler nacks the address. Any other interaction
        /// acknowledges the address.
        handler: W,
    },
}

/// Transaction received from [`SyncI2cTarget::listen_expect_write`] and
/// [`AsyncI2cTarget::listen_expect_write`]
#[must_use = "Implicitly dropping a Transaction will NAK the request"]
pub enum TransactionExpectWrite<R, W> {
    /// A write transaction was received for the expected address, and was used
    /// to fill part of the buffer. All received bytes have been acknowledged.
    ExpectedCompleteWrite { size: usize },
    /// A write transaction was received for the expected address, but is at
    /// least as large as the entire buffer provided. All but the last received
    /// byte has been acknowledged. The provided handler can be used to
    /// acknowledge the last byte of the buffer and receive any further bytes.
    ExpectedPartialWrite { handler: W },
    /// A stop or restart with different address happened since the last
    /// transaction. This may be emitted multiple times between transactions.
    Deselect,
    /// A read transaction has been started and the address byte received but
    /// not yet acknowledged. The address will be acknowledged on the call to
    /// handle_part or handle_complete on the handler. To nack the address,
    /// drop the handler.
    Read {
        /// Address for which the read was received
        address: AnyAddress,
        /// Handler to be used in handling the transaction
        ///
        /// Dropping this handler nacks the address. Any other interaction
        /// acknowledges the address.
        handler: R,
    },
    /// A write transaction has been started for a different address than
    /// expected. The address byte is received but not yet acknowledged. The
    /// address will be acknowledged on the call to handle_part or
    /// handle_complete on the handler. To nack the address, drop the handler.
    Write {
        /// Address for which the write was received
        address: AnyAddress,
        /// Handler to be used in handling the transaction
        ///
        /// Dropping this handler nacks the address. Any other interaction
        /// acknowledges the address.
        handler: W,
    },
}

/// A transaction received from any of the [`SyncI2cTarget`] and [`AsyncI2cTarget`]'s listen functions.
/// This type is intended to be used for simplifying control flow in users of
/// the I2cTarget
pub enum TransactionExpectEither<R, W> {
    /// A read transaction was received for the expected address, and the
    /// entire transaction could be handled using the bytes provided.
    ExpectedCompleteRead { size: usize },
    /// A read transaction was received for the expected address, but more
    /// bytes are needed to complete the transaction.
    ExpectedPartialRead { handler: R },
    /// A write transaction was received for the expected address, and was used
    /// to fill part of the buffer. All received bytes have been acknowledged.
    ExpectedCompleteWrite { size: usize },
    /// A write transaction was received for the expected address, but is at
    /// least as large as the entire buffer provided. All but the last received
    /// byte has been acknowledged. The provided handler can be used to
    /// acknowledge the last byte of the buffer and receive any further bytes.
    ExpectedPartialWrite { handler: W },
    /// A stop or restart with different address happened since the last
    /// transaction. This may be emitted multiple times between transactions.
    Deselect,
    /// A read transaction has been started for a different address than
    /// expected. The address byte is received but not yet acknowledged. The
    /// address will be acknowledged on the call to handle_part or
    /// handle_complete on the handler. To nack the address, drop the handler.
    Read {
        /// Address for which the read was received
        address: AnyAddress,
        /// Handler to be used in handling the transaction
        ///
        /// Dropping this handler nacks the address. Any other interaction
        /// acknowledges the address.
        handler: R,
    },
    /// A write transaction has been started for a different address than
    /// expected. The address byte is received but not yet acknowledged. The
    /// address will be acknowledged on the call to handle_part or
    /// handle_complete on the handler. To nack the address, drop the handler.
    Write {
        /// Address for which the write was received
        address: AnyAddress,
        /// Handler to be used in handling the transaction
        ///
        /// Dropping this handler nacks the address. Any other interaction
        /// acknowledges the address.
        handler: W,
    },
}

impl<R, W> From<Transaction<R, W>> for TransactionExpectRead<R, W> {
    fn from(value: Transaction<R, W>) -> Self {
        match value {
            Transaction::Deselect => Self::Deselect,
            Transaction::Read { address, handler } => Self::Read { address, handler },
            Transaction::Write { address, handler } => Self::Write { address, handler },
        }
    }
}

impl<R, W> From<Transaction<R, W>> for TransactionExpectWrite<R, W> {
    fn from(value: Transaction<R, W>) -> Self {
        match value {
            Transaction::Deselect => Self::Deselect,
            Transaction::Read { address, handler } => Self::Read { address, handler },
            Transaction::Write { address, handler } => Self::Write { address, handler },
        }
    }
}

impl<R, W> From<Transaction<R, W>> for TransactionExpectEither<R, W> {
    fn from(value: Transaction<R, W>) -> Self {
        match value {
            Transaction::Deselect => Self::Deselect,
            Transaction::Read { address, handler } => Self::Read { address, handler },
            Transaction::Write { address, handler } => Self::Write { address, handler },
        }
    }
}

impl<R, W> From<TransactionExpectRead<R, W>> for TransactionExpectEither<R, W> {
    fn from(value: TransactionExpectRead<R, W>) -> Self {
        match value {
            TransactionExpectRead::ExpectedCompleteRead { size } => {
                Self::ExpectedCompleteRead { size }
            }
            TransactionExpectRead::ExpectedPartialRead { handler } => {
                Self::ExpectedPartialRead { handler }
            }
            TransactionExpectRead::Deselect => Self::Deselect,
            TransactionExpectRead::Read { address, handler } => Self::Read { address, handler },
            TransactionExpectRead::Write { address, handler } => Self::Write { address, handler },
        }
    }
}

impl<R, W> From<TransactionExpectWrite<R, W>> for TransactionExpectEither<R, W> {
    fn from(value: TransactionExpectWrite<R, W>) -> Self {
        match value {
            TransactionExpectWrite::ExpectedCompleteWrite { size } => {
                Self::ExpectedCompleteWrite { size }
            }
            TransactionExpectWrite::ExpectedPartialWrite { handler } => {
                Self::ExpectedPartialWrite { handler }
            }
            TransactionExpectWrite::Deselect => Self::Deselect,
            TransactionExpectWrite::Read { address, handler } => Self::Read { address, handler },
            TransactionExpectWrite::Write { address, handler } => Self::Write { address, handler },
        }
    }
}

/// Result of partial handling of a read transaction, see also
/// [`SyncReadTransaction::handle_part`] and
/// [`AsyncReadTransaction::handle_part`]
#[must_use = "Implicitly dropping a Transaction will NAK the request"]
pub enum ReadResult<R> {
    /// The bytes were provided to the master, but more bytes are needed.
    Partial(R),
    /// The transaction was completed, the final read provided usize more
    /// bytes.
    Complete(usize),
}

/// Result of partial handling of a write transaction, see also
/// [`SyncWriteTransaction::handle_part`] and
/// [`AsyncWriteTransaction::handle_part`]
#[must_use = "Implicitly dropping a Transaction will NAK the request"]
pub enum WriteResult<W> {
    /// The buffer was filled with bytes from the master, and it may have
    /// more for us. All but the last byte in the buffer are acknowledged.
    Partial(W),
    /// The transaction was completed, the final write provided usize more
    /// bytes, which were all acknowledged
    Complete(usize),
}

/// I2c device implementing I2c target functionality in a synchronous fashion.
pub trait SyncI2cTarget {
    type Error;
    type Read<'a>: SyncReadTransaction<Error = Self::Error> + 'a
    where
        Self: 'a;
    type Write<'a>: SyncWriteTransaction<Error = Self::Error> + 'a
    where
        Self: 'a;

    /// Listen for a new transaction to occur
    fn listen(&mut self) -> Result<Transaction<Self::Read<'_>, Self::Write<'_>>, Self::Error>;

    /// Listen for a new transaction to occur, expecting a write. Using this
    /// function may allow some hardware to handle the write more efficiently.
    fn listen_expect_write<'a>(
        &'a mut self,
        expected_address: AnyAddress,
        write_buffer: &mut [u8],
    ) -> Result<TransactionExpectWrite<Self::Read<'a>, Self::Write<'a>>, Self::Error> {
        match self.listen()? {
            Transaction::Write { address, handler } if address == expected_address => {
                match handler.handle_part(write_buffer)? {
                    WriteResult::Complete(size) => {
                        Ok(TransactionExpectWrite::ExpectedCompleteWrite { size })
                    }
                    WriteResult::Partial(handler) => {
                        Ok(TransactionExpectWrite::ExpectedPartialWrite { handler })
                    }
                }
            }
            other => Ok(other.into()),
        }
    }
    /// Listen for a new transaction to occur, expecting a read. Using this
    /// function may allow some hardware to handle the read more efficiently.
    fn listen_expect_read<'a>(
        &'a mut self,
        expected_address: AnyAddress,
        read_buffer: &[u8],
    ) -> Result<TransactionExpectRead<Self::Read<'a>, Self::Write<'a>>, Self::Error> {
        match self.listen()? {
            Transaction::Read { address, handler } if address == expected_address => {
                match handler.handle_part(read_buffer)? {
                    ReadResult::Complete(size) => {
                        Ok(TransactionExpectRead::ExpectedCompleteRead { size })
                    }
                    ReadResult::Partial(handler) => {
                        Ok(TransactionExpectRead::ExpectedPartialRead { handler })
                    }
                }
            }
            other => Ok(other.into()),
        }
    }
}

/// Handler for a synchronous read transaction
///
/// On drop, will set the hardware to provide an implementation-defined overrun
/// character for the rest of the read. If the address was not yet
/// acknowledged, dropping will nack the address.
pub trait SyncReadTransaction: Sized {
    type Error;
    /// Provide the next buffer to send to the master as part of the read
    /// transaction, keeping the option open for providing even more data
    /// should this not be sufficient.
    fn handle_part(self, buffer: &[u8]) -> Result<ReadResult<Self>, Self::Error>;

    /// Send the buffer to the master as part of the read transaction, then
    /// complete it by providing the overrun character for the remainder of the
    /// read transaction until the master ends it.
    ///
    /// Implementations may want to override the default implementation to
    /// provide better performance.
    fn handle_complete(self, buffer: &[u8], ovc: u8) -> Result<usize, Self::Error> {
        match self.handle_part(buffer)? {
            ReadResult::Complete(size) => Ok(size),
            ReadResult::Partial(mut this) => {
                let mut total = buffer.len();
                loop {
                    match this.handle_part(&[ovc])? {
                        ReadResult::Complete(extra) => break Ok(total + extra),
                        ReadResult::Partial(handler) => {
                            this = handler;
                            total += 1;
                        }
                    }
                }
            }
        }
    }
}

/// Handler for a synchronous write transaction
///
/// On drop, will nack the last byte and end the transaction
pub trait SyncWriteTransaction: Sized {
    type Error;

    /// Accept buffer.len bytes of the write, acknowledging all but the last
    /// byte. The last byte is neither acknowledged nor not acknowledged.
    fn handle_part(self, buffer: &mut [u8]) -> Result<WriteResult<Self>, Self::Error>;

    /// Accept buffer.len bytes of the write, acknowledging all these bytes.
    /// Should the master try to send more bytes than fit in the buffer, any
    /// overrun is not acknowledged.
    ///
    /// Implementations may want to override the default implementation to
    /// provide better performance.
    fn handle_complete(self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        match self.handle_part(buffer)? {
            WriteResult::Complete(size) => Ok(size),
            WriteResult::Partial(handler) => {
                // Ensure the last byte is acknowledged.
                let _ = handler.handle_part(&mut [0])?;
                Ok(buffer.len())
            }
        }
    }
}

/// I2c device implementing I2c target functionality for async runtimes.
pub trait AsyncI2cTarget {
    type Error;
    type Read<'a>: AsyncReadTransaction<Error = Self::Error> + 'a
    where
        Self: 'a;
    type Write<'a>: AsyncWriteTransaction<Error = Self::Error> + 'a
    where
        Self: 'a;

    /// Listen for a new transaction to occur
    async fn listen(&mut self)
    -> Result<Transaction<Self::Read<'_>, Self::Write<'_>>, Self::Error>;

    /// Listen for a new transaction to occur, expecting a write. Using this
    /// function may allow some hardware to handle the write more efficiently.
    async fn listen_expect_write<'a>(
        &'a mut self,
        expected_address: AnyAddress,
        write_buffer: &mut [u8],
    ) -> Result<TransactionExpectWrite<Self::Read<'a>, Self::Write<'a>>, Self::Error> {
        match self.listen().await? {
            Transaction::Write { address, handler } if address == expected_address => {
                match handler.handle_part(write_buffer).await? {
                    WriteResult::Complete(size) => {
                        Ok(TransactionExpectWrite::ExpectedCompleteWrite { size })
                    }
                    WriteResult::Partial(handler) => {
                        Ok(TransactionExpectWrite::ExpectedPartialWrite { handler })
                    }
                }
            }
            other => Ok(other.into()),
        }
    }
    /// Listen for a new transaction to occur, expecting a read. Using this
    /// function may allow some hardware to handle the read more efficiently.
    async fn listen_expect_read<'a>(
        &'a mut self,
        expected_address: AnyAddress,
        read_buffer: &[u8],
    ) -> Result<TransactionExpectRead<Self::Read<'a>, Self::Write<'a>>, Self::Error> {
        match self.listen().await? {
            Transaction::Read { address, handler } if address == expected_address => {
                match handler.handle_part(read_buffer).await? {
                    ReadResult::Complete(size) => {
                        Ok(TransactionExpectRead::ExpectedCompleteRead { size })
                    }
                    ReadResult::Partial(handler) => {
                        Ok(TransactionExpectRead::ExpectedPartialRead { handler })
                    }
                }
            }
            other => Ok(other.into()),
        }
    }
}

/// Handler for an asynchronous read transaction
///
/// On drop, will set the hardware to provide an implementation-defined overrun
/// character for the rest of the read. If the address was not yet
/// acknowledged, dropping will nack the address.
pub trait AsyncReadTransaction: Sized {
    type Error;
    /// Provide the next buffer to send to the master as part of the read
    /// transaction, keeping the option open for providing even more data
    /// should this not be sufficient.
    async fn handle_part(self, buffer: &[u8]) -> Result<ReadResult<Self>, Self::Error>;

    /// Send the buffer to the master as part of the read transaction, then
    /// complete it by providing the overrun character for the remainder of the
    /// read transaction until the master ends it.
    ///
    /// Implementations may want to override the default implementation to
    /// provide better performance.
    async fn handle_complete(self, buffer: &[u8], ovc: u8) -> Result<usize, Self::Error> {
        match self.handle_part(buffer).await? {
            ReadResult::Complete(size) => Ok(size),
            ReadResult::Partial(mut this) => {
                let mut total = buffer.len();
                loop {
                    match this.handle_part(&[ovc]).await? {
                        ReadResult::Complete(extra) => break Ok(total + extra),
                        ReadResult::Partial(handler) => {
                            this = handler;
                            total += 1;
                        }
                    }
                }
            }
        }
    }
}

/// Handler for an asynchronous write transaction
///
/// On drop, will nack the last byte and end the transaction
pub trait AsyncWriteTransaction: Sized {
    type Error;

    /// Accept buffer.len bytes of the write, acknowledging all but the last
    /// byte. The last byte is neither acknowledged nor not acknowledged.
    async fn handle_part(self, buffer: &mut [u8]) -> Result<WriteResult<Self>, Self::Error>;

    /// Accept buffer.len bytes of the write, acknowledging all these bytes.
    /// Should the master try to send more bytes than fit in the buffer, any
    /// overrun is not acknowledged.
    ///
    /// Implementations may want to override the default implementation to
    /// provide better performance.
    async fn handle_complete(self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        match self.handle_part(buffer).await? {
            WriteResult::Complete(size) => Ok(size),
            WriteResult::Partial(handler) => {
                // Ensure the last byte is acknowledged.
                let _ = handler.handle_part(&mut [0]).await?;
                Ok(buffer.len())
            }
        }
    }
}
