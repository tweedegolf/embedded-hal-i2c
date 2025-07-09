# Research

The design of the proposed trait is the result of a review of existing I2C target implementations across the embedded
Rust ecosystem.
We compared their various API designs, to identify common patterns.
In this document, we categorize these existing solutions to show possible forms of the new trait.

## Simple listen

The user has to listen for a command by the controller and then is free to respond any way they like:

```rust
/// As implemented in embassy-imxrt, stm32g0xx_hal, va108xx-rs, rp2040-hal, rp235x-hal, (nrf-hal-common)
pub trait Simple {
    fn listen(&mut self) -> Result<SimpleEvent, Error>;
    fn respond_to_read(&mut self, data: &[u8], fill: u8) -> Result<(), Error>;
    fn respond_to_write(&mut self, data: &mut [u8]) -> Result<(), Error>;
}

pub enum SimpleEvent {
    /// The controller requests data.
    ReadRequested,
    /// The controller sends data.
    WriteRequested,

    // imxrt
    Probe,

    // rp2040-hal and rp235x-hal
    /// Start condition has been detected.
    Start,
    /// Restart condition has been detected.
    Restart,
    /// Stop condition detected.
    Stop,
}
```

### Pros

* Very simple interface
* No restrictions for partial read/write
* Clear communication of Start, Restart, Stop events

### Cons

* No compile time check for correct response

### Implementations

<details><summary>embassy-imxrt</summary>

[Source](https://github.com/OpenDevicePartnership/embassy-imxrt/blob/ce3db77e1364d9fdfee639ec94f4c7f3b3a870b1/src/i2c/slave.rs#L271)

```rust
impl I2cSlave {
    /// Listen for commands from the I2C Master.
    pub fn listen(&self) -> Result<Command> { ... }

    /// Respond to write command from  master
    pub fn respond_to_write(&self, buf: &mut [u8]) -> Result<Response> { ... }

    /// Respond to read command from  master
    pub fn respond_to_read(&self, buf: &[u8]) -> Result<Response> { ... }

    /// Listen for commands from the I2C Master asynchronously
    pub async fn listen(&mut self) -> Result<Command> { ... }

    /// Respond to write command from master
    pub async fn respond_to_write(&mut self, buf: &mut [u8]) -> Result<Response> { ... }

    /// Respond to read command from master
    /// User must provide enough data to complete the transaction or else
    ///    we will get stuck in this function
    pub async fn respond_to_read(&mut self, buf: &[u8]) -> Result<Response> { ... }x
}

/// Command from master
pub enum Command {
    /// I2C probe with no data
    Probe,

    /// I2C Read
    Read,

    /// I2C Write
    Write,
}

/// Result of response functions
pub enum Response {
    /// I2C transaction complete with this amount of bytes
    Complete(usize),

    /// I2C transaction pending with this amount of bytes completed so far
    Pending(usize),
}

/// shorthand for -> `Result<T>`
pub type Result<T> = core::result::Result<T, Error>;

/// Error information type
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// configuration requested is not supported
    UnsupportedConfiguration,

    /// transaction failure types
    Transfer(TransferError),
}
```

</details>

<details><summary>stm32g0xx_hal</summary>

[From](https://github.com/stm32-rs/stm32g0xx-hal/blob/6aa9567e0b019184ee8013242da3dcd721ead600/src/i2c/blocking.rs#L11C1-L35C2)

```rust
pub trait I2cSlave {
    /// Enable/Disable Slave Byte Control. Default SBC is switched on.
    /// For master write/read the transaction should start with sbc disabled.
    /// So ACK will be send on the last received byte.
    /// Before the send phase SBC should be enabled again.
    fn slave_sbc(&mut self, sbc_enabled: bool);

    /// An optional tuple is returned with the address as sent by the master. The address is for 7 bit in range of 0..127
    fn slave_addressed(&mut self) -> Result<Option<(u16, I2cDirection)>, Error>;

    /// Wait until this slave is addressed by the master.
    /// A tuple is returned with the address as sent by the master. The address is for 7 bit in range of 0..127
    fn slave_wait_addressed(&mut self) -> Result<(u16, I2cDirection), Error>;

    /// Start reading the bytes, send by the master . If OK returned, all bytes are transferred
    /// If the master want to send more bytes than the slave can recieve the slave will NACK the n+1 byte
    /// In this case the function will return IncorrectFrameSize(bytes.len() + 1)
    /// If the master did send a STOP before all bytes are recieve, the slave will return IncorrectFrameSize(actual nr of bytes send)
    fn slave_read(&mut self, bytes: &mut [u8]) -> Result<(), Error>;

    /// Start writing the bytes, the master want to receive. If OK returned, all bytes are transferred
    /// If the master wants more data than bytes.len()  the master will run into a timeout, This function will return Ok(())
    /// If the master wants less data than bytes.len(), the function will return  IncorrectFrameSize(bytes.len() + 1)
    fn slave_write(&mut self, bytes: &[u8]) -> Result<(), Error>;
}

/// The MasterWriteSlaveRead  is fully under control of the master. The slave simply has to accept
/// the amount of bytes send by the master
/// The MasterReadSlaveWrite is onder control of the slave. The slave decides how many bytes to send
pub trait I2cSlave {
    /// Enable/ disable sbc. Default sbc is switched on.
    /// For master write/read the transaction should start with sbc disabled.
    /// So ACK will be send on the last received byte. Then before the send phase sbc should enabled again
    fn slave_sbc(&mut self, sbc_enabled: bool);

    /// Start writing the bytes, the master want to receive. If OK returned, all bytes are transferred
    /// If the master wants more data than bytes.len()  the master will run into a timeout, This function will return Ok(())
    /// If the master wants less data than bytes.len(), this function will return OK, but with the incorrect nr
    /// of bytes  in the I2cResult
    /// Note that this function must be called after a I2cResult::Addressed when MasterReadSlaveWrite
    /// otherwise the bus gets blocked.
    fn slave_write(&mut self, bytes: &[u8]) -> Result<(), Error>;

    /// return the address of the addressed slave
    fn get_address(&self) -> u16;

    /// return a non mutable slice to the internal data, with the size of the last transaction
    fn get_data(&self) -> &[u8];

    /// Set and enable the (7 bit) adress. To keep the interface generic, only slave address 1 can be set
    fn set_address(&mut self, address: u16);
}

/// I2C error
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Overrun,
    Nack,
    PECError,
    BusError,
    ArbitrationLost,
    IncorrectFrameSize(usize),
}
```

</details>

<details><summary>va108xx-rs</summary>

[From](https://egit.irs.uni-stuttgart.de/rust/va108xx-rs/src/branch/main/va108xx-hal/src/i2c.rs#L724)

```rust
impl<Addr> I2cSlave<..., Addr> {
    pub fn enable_slave(self) -> Self { ... }
    pub fn disable_slave(self) -> Self { ... }

    /// Get the last address that was matched by the slave control and the corresponding
    /// master direction
    pub fn last_address(&self) -> (I2cDirection, u32) { ... }

    pub fn write(&mut self, output: &[u8]) -> Result<(), Error> { ... }

    pub fn read(&mut self, buffer: &mut [u8]) -> Result<(), Error> { ... }
}
```

</details>

<details><summary> rp2040-hal and rp235x-hal</summary>

[Source](https://docs.rs/rp2040-hal/latest/rp2040_hal/i2c/struct.I2C.html#impl-I2C%3CT,+PINS,+Peripheral%3E)

```rust
impl I2C<..., Peripheral> {
    /// Push up to `usize::min(TX_FIFO_SIZE, buf.len())` bytes to the TX FIFO.
    /// Returns the number of bytes pushed to the FIFO. Note this does *not* reflect how many bytes
    /// are effectively received by the controller.
    pub fn write(&mut self, buf: &[u8]) -> usize { ... }

    /// Pull up to `usize::min(RX_FIFO_SIZE, buf.len())` bytes from the RX FIFO.
    pub fn read(&mut self, buf: &mut [u8]) -> usize { ... }

    /// Returns the next i2c event if any.
    pub fn next_event(&mut self) -> Option<Event> { ... }

    /// Asynchronously waits for an Event.
    pub async fn wait_next(&mut self) -> Event { ... }
}

/// This allows I2C to be used with `core::iter::Extend`.
impl Iterator for I2C<..., Peripheral> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> { ... }
}

/// I2C bus events
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Event {
    /// Start condition has been detected.
    Start,
    /// Restart condition has been detected.
    Restart,
    /// The controller requests data.
    TransferRead,
    /// The controller sends data.
    TransferWrite,
    /// Stop condition detected.
    Stop,
}
```

</details>

<details><summary>nrf-hal-common</summary>

[From](https://github.com/nrf-rs/nrf-hal/blob/master/nrf-hal-common/src/twis.rs)

```rust
impl Twis {
    /// Configures secondary I2C address.
    pub fn set_address1(&self, address1: u8) -> &Self { ... }

    /// Sets the over-read character (character sent on over-read of the transmit buffer).
    pub fn set_orc(&self, orc: u8) -> &Self { ... }

    // ...

    /// Stops the TWI transaction and waits until it has stopped.
    pub fn stop(&self) -> &Self { ... }

    // ...

    /// Returns matched address for latest command.
    pub fn address_match(&self) -> u8 { ... }

    // ...

    /// Checks if the TWI transaction is done.
    pub fn is_done(&self) -> bool { ... }

    /// Returns number of bytes received in last granted transaction.
    pub fn amount(&self) -> u32 { ... }

    /// Checks if RX buffer overflow was detected.
    pub fn is_overflow(&self) -> bool { ... }

    /// Checks if NACK was sent after receiving a data byte.
    pub fn is_data_nack(&self) -> bool { ... }

    /// Checks if TX buffer over-read was detected and ORC was clocked out.
    pub fn is_overread(&self) -> bool { ... }

    // ommited (PPI stuff)

    /// Write to an I2C controller.
    ///
    /// The buffer must reside in RAM and have a length of at most
    /// 255 bytes on the nRF52832 and at most 65535 bytes on the nRF52840.
    pub fn tx_blocking(&mut self, buffer: &[u8]) -> Result<(), Error> { ... }


    /// Read from an I2C controller.
    ///
    /// The buffer must have a length of at most 255 bytes on the nRF52832
    /// and at most 65535 bytes on the nRF52840.
    pub fn rx_blocking(&mut self, buffer: &mut [u8]) -> Result<(), Error> { ... }

    /// Receives data into the given `buffer`. Buffer must be located in RAM.
    /// Returns a value that represents the in-progress DMA transfer.
    pub fn rx<W, B>(self, mut buffer: B) -> Result<Transfer<T, B>, Error>
    where
        B: WriteBuffer<Word=W> + 'static,
    { ... }


    /// Transmits data from the given `buffer`. Buffer must be located in RAM.
    /// Returns a value that represents the in-progress DMA transfer.
    pub fn tx<W, B>(self, buffer: B) -> Result<Transfer<T, B>, Error>
    where
        B: ReadBuffer<Word=W> + 'static,
    { ... }
}

/// A DMA transfer
pub struct Transfer<T: Instance, B> {
    ...
}
impl Transfer<...> {
    /// Blocks until the transaction is done and returns the buffer.
    pub fn wait(mut self) -> (B, Twis<T>) { ... }

    /// Checks if the granted transaction is done.
    pub fn is_done(&mut self) -> bool { ... }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Error {
    TxBufferTooLong,
    RxBufferTooLong,
    Transmit,
    Receive,
    DMABufferNotInDataMemory,
    DataNack,
    OverFlow,
    OverRead,
}
```

</details>

<details><summary>esp-idf-hal</summary>

[From](https://docs.esp-rs.org/esp-idf-hal/esp_idf_hal/i2c/struct.I2cSlaveDriver.html)

```rust
#[cfg(not(esp32c2))]
impl I2cSlaveDriver {
    pub fn read(&mut self, buffer: &mut [u8], timeout: TickType_t) -> Result<usize, EspError> { ... }
    pub fn write(&mut self, bytes: &[u8], timeout: TickType_t) -> Result<usize, EspError> { ... }
}
```

</details>

## Listen and Write in one

Same as above but `listen` always gets a buffer in case a write happens.

```rust
/// As implemented in embassy-rp, embassy-nrf
pub trait WriteByListen {
    fn listen(&mut self, write_buffer: &mut [u8]) -> Result<SimpleEvent, Error>;
    fn respond_to_read(&mut self, data: &[u8], fill: u8) -> Result<(), Error>;
}

pub enum WrityByListenEvent {
    /// Read
    Read,
    /// Write+read
    WriteRead(usize),
    /// Write
    Write(usize),

    // For embassy-rp
    /// General Call
    GeneralCall(usize),
}
```

### Pros

* Can not respond with a read to a write
* Models typical, write address, read register use nicely (ReadWrite)

### Cons

* Can still `respond_to_read` without read being requested
* Write buffer always needs to hold the complete data to be written

### Implementations

<details><summary>embassy-rp</summary>

[Source](https://docs.rs/embassy-rp/0.4.0/embassy_rp/i2c_slave/struct.I2cSlave.html)

```rust
impl I2cSlave {
    /// Reset the i2c peripheral. If you cancel a respond_to_read, you may stall the bus.
    /// You can recover the bus by calling this function, but doing so will almost certainly cause
    /// an i/o error in the master.
    pub fn reset(&mut self) { ... }


    /// Wait asynchronously for commands from an I2C master.
    /// `buffer` is provided in case master does a 'write', 'write read', or 'general call' and is unused for 'read'.
    pub async fn listen(&mut self, buffer: &mut [u8]) -> Result<Command, Error> { ... }

    /// Respond to an I2C master READ command, asynchronously.
    pub async fn respond_to_read(&mut self, buffer: &[u8]) -> Result<ReadStatus, Error> { ... }


    /// Respond to reads with the fill byte until the controller stops asking
    pub async fn respond_till_stop(&mut self, fill: u8) -> Result<(), Error> { ... }


    /// Respond to a master read, then fill any remaining read bytes with `fill`
    pub async fn respond_and_fill(&mut self, buffer: &[u8], fill: u8) -> Result<ReadStatus, Error> { ... }
}

/// Received command
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Command {
    /// General Call
    GeneralCall(usize),
    /// Read
    Read,
    /// Write+read
    WriteRead(usize),
    /// Write
    Write(usize),
}

/// Possible responses to responding to a read
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ReadStatus {
    /// Transaction Complete, controller naked our last byte
    Done,
    /// Transaction Incomplete, controller trying to read more bytes than were provided
    NeedMoreBytes,
    /// Transaction Complere, but controller stopped reading bytes before we ran out
    LeftoverBytes(u16),
}

/// I2C error
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum Error {
    /// I2C abort with error
    Abort(AbortReason),
    /// User passed in a response buffer that was 0 length
    InvalidResponseBufferLength,
    /// The response buffer length was too short to contain the message
    ///
    /// The length parameter will always be the length of the buffer, and is
    /// provided as a convenience for matching alongside `Command::Write`.
    PartialWrite(usize),
    /// The response buffer length was too short to contain the message
    ///
    /// The length parameter will always be the length of the buffer, and is
    /// provided as a convenience for matching alongside `Command::GeneralCall`.
    PartialGeneralCall(usize),
}
```

</details>

<details><summary>embassy-nrf</summary>

[Source](https://docs.rs/embassy-nrf/0.3.1/embassy_nrf/twis/struct.Twis.html)

```rust
impl Twis {
    /// Returns matched address for latest command.
    pub fn address_match(&self) -> u8 { ... }

    /// Returns the index of the address matched in the latest command.
    pub fn address_match_index(&self) -> usize { ... }


    /// Wait for commands from an I2C master.
    /// `buffer` is provided in case master does a 'write' and is unused for 'read'.
    /// The buffer must have a length of at most 255 bytes on the nRF52832
    /// and at most 65535 bytes on the nRF52840.
    /// To know which one of the addresses were matched, call `address_match` or `address_match_index`
    pub fn blocking_listen(&mut self, buffer: &mut [u8]) -> Result<Command, Error> { ... }


    /// Respond to an I2C master READ command.
    /// Returns the number of bytes written.
    /// The buffer must have a length of at most 255 bytes on the nRF52832
    /// and at most 65535 bytes on the nRF52840.
    pub fn blocking_respond_to_read(&mut self, buffer: &[u8]) -> Result<usize, Error> { ... }


    /// Same as [`blocking_respond_to_read`](Twis::blocking_respond_to_read) but will fail instead of copying data into RAM.
    /// Consult the module level documentation to learn more.
    pub fn blocking_respond_to_read_from_ram(&mut self, buffer: &[u8]) -> Result<usize, Error> { ... }

    /// Wait for commands from an I2C master, with timeout.
    /// `buffer` is provided in case master does a 'write' and is unused for 'read'.
    /// The buffer must have a length of at most 255 bytes on the nRF52832
    /// and at most 65535 bytes on the nRF52840.
    /// To know which one of the addresses were matched, call `address_match` or `address_match_index`
    #[cfg(feature = "time")]
    pub fn blocking_listen_timeout(&mut self, buffer: &mut [u8], timeout: Duration) -> Result<Command, Error> { ... }

    /// Respond to an I2C master READ command with timeout.
    /// Returns the number of bytes written.
    /// See [`blocking_respond_to_read`].
    #[cfg(feature = "time")]
    pub fn blocking_respond_to_read_timeout(&mut self, buffer: &[u8], timeout: Duration) -> Result<usize, Error> { ... }

    /// Same as [`blocking_respond_to_read_timeout`](Twis::blocking_respond_to_read_timeout) but will fail instead of copying data into RAM.
    /// Consult the module level documentation to learn more.
    #[cfg(feature = "time")]
    pub fn blocking_respond_to_read_from_ram_timeout(
        &mut self,
        buffer: &[u8],
        timeout: Duration,
    ) -> Result<usize, Error> { ... }

    /// Wait asynchronously for commands from an I2C master.
    /// `buffer` is provided in case master does a 'write' and is unused for 'read'.
    /// The buffer must have a length of at most 255 bytes on the nRF52832
    /// and at most 65535 bytes on the nRF52840.
    /// To know which one of the addresses were matched, call `address_match` or `address_match_index`
    pub async fn listen(&mut self, buffer: &mut [u8]) -> Result<Command, Error> { ... }

    /// Respond to an I2C master READ command, asynchronously.
    /// Returns the number of bytes written.
    /// The buffer must have a length of at most 255 bytes on the nRF52832
    /// and at most 65535 bytes on the nRF52840.
    pub async fn respond_to_read(&mut self, buffer: &[u8]) -> Result<usize, Error> { ... }

    /// Same as [`respond_to_read`](Twis::respond_to_read) but will fail instead of copying data into RAM. Consult the module level documentation to learn more.
    pub async fn respond_to_read_from_ram(&mut self, buffer: &[u8]) -> Result<usize, Error> { ... }
}

/// Received command
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Command {
    /// Read
    Read,
    /// Write+read
    WriteRead(usize),
    /// Write
    Write(usize),
}

/// TWIS error.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum Error {
    /// TX buffer was too long.
    TxBufferTooLong,
    /// RX buffer was too long.
    RxBufferTooLong,
    /// Didn't receive an ACK bit after a data byte.
    DataNack,
    /// Bus error.
    Bus,
    /// The buffer is not in data RAM. It's most likely in flash, and nRF's DMA cannot access flash.
    BufferNotInRAM,
    /// Overflow
    Overflow,
    /// Overread
    OverRead,
    /// Timeout
    Timeout,
}
```

</details>

## Typed Listen

In this case there is only a single `listen` command. The type it returns lets the user respond to that specific event.

```rust
#![allow(async_fn_in_trait)]

// Review note: Existing trait from embedded_hal crates, provided
// here for ease of experimenting
pub trait AddressMode: Eq {}

// General review note: The variation presented here has all of the behavior specified. It is possible
// to leave more of the behavior around when things are (n)acked implementation-defined.

// Review note: As a minor variation, this type could if desired be split into multiple to better match what each
// type of listen function may actually return.
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
    /// Only returned when an expected read was provided to the listen
    /// function. The read transaction has completed and size bytes were
    /// provided to the master.
    ExpectedCompleteReadTransaction { address: A, size: usize },
    /// Only returned when an expected write was provided to the listen
    /// function. The write transaction has completed and size bytes were
    /// received from the master.
    ExpectedCompleteWriteTransaction { address: A, size: usize },
}

pub trait I2CSlave<A: AddressMode> {
    type Error;
    // Review note: Different error types for read and write transactions could
    // be interesting, but would result in either an Into bound in order for the
    // listen_expect_* functions to be able to be provided.
    type Read<'a>: ReadTransaction<Error=Self::Error> + 'a
    where
        Self: 'a;
    type Write<'a>: WriteTransaction<Error=Self::Error> + 'a
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
    ) -> Result<Transaction<A, Self::Read<'a>, Self::Write<'a>>, Self::Error> {
        match self.listen().await? {
            result @ Transaction::ReadTransaction { .. } => Ok(result),
            Transaction::WriteTransaction { address, handler } => {
                if address == expected_address {
                    match handler.handle_part(write_buffer).await? {
                        WriteResult::Finished(size) => {
                            Ok(Transaction::ExpectedCompleteWriteTransaction { address, size })
                        }
                        WriteResult::PartialComplete(handler) => {
                            Ok(Transaction::WriteTransaction { address, handler })
                        }
                    }
                } else {
                    Ok(Transaction::WriteTransaction { address, handler })
                }
            }
            _ => panic!("Listen must not return an ExpectedComplete"),
        }
    }
    /// Listen for a new transaction to occur, expecting a read
    fn listen_expect_read<'a>(
        &'a mut self,
        expected_address: A,
        read_buffer: &[u8],
    ) -> Result<Transaction<A, Self::Read<'a>, Self::Write<'a>>, Self::Error>;
    /// Listen for a new transaction to occur, expecting either
    fn listen_expect_either<'a>(
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
    async fn handle_part(
        self,
        buffer: &[u8],
    ) -> Result<ReadResult<Self>, Self::Error>;
    /// Finish the entire read transaction, providing the overrun character once the buffer runs out
    async fn handle_complete(
        self,
        buffer: &[u8],
        ovc: u8,
    ) -> Result<usize, Self::Error> {
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
    async fn handle_part(
        self,
        buffer: &mut [u8],
    ) -> Result<WriteResult<Self>, Self::Error>;
    /// Accept buffer.len bytes of the write, acknowledging all but the last byte, and nacking on the last byte.
    async fn handle_complete(
        self,
        buffer: &mut [u8],
    ) -> Result<usize, Self::Error> {
        match self.handle_part(buffer).await? {
            WriteResult::Finished(size) => Ok(size),
            WriteResult::PartialComplete(handler) => {
                drop(handler); // sends the nack
                Ok(buffer.len())
            }
        }
    }
}
```

### Pros

* Compile time checked correct responses
* Reads and Writes can be handled in chunks

### Cons

* More complex types, especially as a `trait`

### Implementations

<details><summary>lpc8xx-hal</summary>

[From](https://github.com/lpc-rs/lpc8xx-hal/blob/master/src/i2c/slave.rs)

```rust
impl Slave<...> {
    /// Wait until software intervention is required
    ///
    /// The returned enum indicates the current state. Each variant provides an
    /// API to react to that state.
    pub fn wait(&mut self) -> nb::Result<State<I>, Error> { ... }
}

/// The current state of the slave
///
/// Each variant provides an API to react to that state. Call [`I2C::wait`] to
/// get an instance of this struct.
///
/// [`I2c::wait`]: ../struct.I2C.html#method.wait
pub enum State<'r, I: Instance> {
    /// Address sent by master has been matched
    AddressMatched(AddressMatched<'r, I>),

    /// Data has been received from master
    RxReady(RxReady<'r, I>),

    /// Ready to transmit data to master
    TxReady(TxReady<'r, I>),
}

/// API for handling the "address matched" state
///
/// You can gain access to this API through [`State`].
///
/// [`State`]: enum.State.html
pub struct AddressMatched<'r, I: Instance> {
    ...
}
impl AddressMatched<...> {
    /// Return the received address
    pub fn address(&self) -> Result<u8, Error> { ... }

    /// Acknowledge the matched address
    pub fn ack(self) -> Result<(), Error> { ... }

    /// Reject the matched address
    pub fn nack(self) -> Result<(), Error> { ... }
}

/// API for handling the "data received" state
///
/// You can gain access to this API through [`State`].
///
/// [`State`]: enum.State.html
pub struct RxReady<'r, I: Instance> {
    ...
}
impl RxReady<'_, ...> {
    /// Read the available data
    ///
    /// If you call this method multiple times, the same data will be returned
    /// each time. To receive the next byte, acknowledge the current one using
    /// [`ack`], then call [`I2C::wait`] again.
    ///
    /// [`ack`]: #method.ack
    /// [`I2C::wait`]: ../struct.I2C.html#method.wait
    pub fn read(&self) -> Result<u8, Error> { ... }

    /// Acknowledge the received data
    pub fn ack(self) -> Result<(), Error> { ... }

    /// Reject the received data
    pub fn nack(self) -> Result<(), Error> { ... }
}

/// API for handling the "ready to transmit" state
///
/// You can gain access to this API through [`State`].
///
/// [`State`]: enum.State.html
pub struct TxReady<'r, I: Instance> {
    ...
}
impl TxReady<...> {
    /// Transmit data
    pub fn transmit(self, data: u8) -> Result<(), Error> { ... }
}

// NOTE: Shared with master state
/// I2C error
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// Event Timeout
    ///
    /// Corresponds to the EVENTTIMEOUT flag in the STAT register.
    EventTimeout,

    /// Master Arbitration Loss
    ///
    /// Corresponds to the MSTARBLOSS flag in the STAT register.
    MasterArbitrationLoss,

    /// Master Start/Stop Error
    ///
    /// Corresponds to the MSTSTSTPERR flag in the STAT register.
    MasterStartStopError,

    /// Monitor Overflow
    ///
    /// Corresponds to the MONOV flag in the STAT register.
    MonitorOverflow,

    /// SCL Timeout
    ///
    /// Corresponds to the SCLTIMEOUT flag in the STAT register.
    SclTimeout,

    /// The I2C code encountered an unexpected hardware state
    UnexpectedState {
        /// The state that was expected
        expected: master::State,

        /// The state that was actually set
        ///
        /// The `Ok` variant represents a valid state. The `Err` variant
        /// represents an invalid bit pattern in the MSTSTATE field.
        actual: Result<master::State, u8>,
    },

    /// An unencodable address was specified.
    ///
    /// Currently, only seven-bit addressing is implemented.
    AddressOutOfRange,

    /// While in slave mode, an unknown state was detected
    UnknownSlaveState(u8),
}
```

</details>

## Callback

In this case the driver is control, in case an event happens, the user provided callback handles how to respond to it.

### Pros

* Quick responses guranteed, since callback is not async
* (?) Could be handled in an interrupt context, avoiding clock-stretching even more
* Compile time checked correct response

### Cons

* User needs to implement a state-machine "by-hand"
* HAL has to manage buffers
* No simple way to specify a fill character

### Implementations

Only implemented in `embassy-npcx`.

[Source](https://github.com/OpenDevicePartnership/embassy-npcx/blob/2e60b03e0bf4068ef02dead6444bea9c3a9b7cdf/src/i2c.rs#L1147)

```rust
// NOTE: This combines master and slave in a single peripheral
impl I2CController {
    /// Listen for i2c interactions targeting the specified addresses. The handler will be called
    /// to handle the various transactions. The listening can be stopped by calling the [CancellationToken::cancel] function
    /// on the given token
    pub async fn listen(
        &mut self,
        addresses: &[u8],
        mut handler: impl FnMut(u8, ListenCommand),
        cancellation_token: &CancellationToken,
    ) -> Result<(), ListenError> { ... }
}
```

<details><summary>Types</summary>

```rust
/// Commands the user has to handle in the listen code
#[derive(Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ListenCommand<'a> {
    /// Part of a master's write has been received
    PartialWrite(&'a [u8]),
    /// The master's write has been completed.
    WriteFinished,
    /// Prepare a buffer for a read operation. Note, not
    /// all bytes in the buffer may be read. Actions
    /// that should happen once bytes have been read should
    /// be done in ReadFinished.
    PrepareRead(&'a mut [u8]),
    /// A read has been finished. Reports how many bytes were
    /// read in total as part of the read.
    ReadFinished(usize),
    /// A bus error occured. No action is needed, but gives
    /// the handler a chance to report this. Always occurs with
    /// address 0. The corresponding finished command for the
    /// transaction will still be provided after the bus error.
    BusError,
}


/// Error type for [I2CController::listen]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ListenError {
    /// The given list of addresses has something wrong with it
    InvalidAddressList,
}

/// A token to cancel an operation with
pub struct CancellationToken {
    ...
}
```

[CancellationToken source](https://github.com/OpenDevicePartnership/embassy-npcx/blob/main/src/cancellation.rs#L7C1-L8C31)

</details>
