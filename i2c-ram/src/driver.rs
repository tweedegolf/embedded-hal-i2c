use embedded_hal_i2c::{AddressMode, AsyncI2cController, Error as _, ErrorKind, Operation};

pub struct I2cRam<I, A> {
    i2c: I,
    address: A,
}

impl<I, A> I2cRam<I, A>
where
    I: AsyncI2cController<A>,
    A: AddressMode + Copy,
{
    pub const fn new(i2c: I, address: A) -> I2cRam<I, A> {
        I2cRam { i2c, address }
    }

    pub async fn read(&mut self, address: u16, buf: &mut [u8]) -> Result<(), Error<I::Error>> {
        self.i2c
            .write_read(self.address, &address.to_le_bytes(), buf)
            .await
            .map_err(|e| match e.kind() {
                ErrorKind::NoAcknowledge(_) => Error::OutOfBounds,
                _ => Error::I2c(e),
            })
    }

    pub async fn write(&mut self, address: u16, buf: &[u8]) -> Result<(), Error<I::Error>> {
        let mem_address = address.to_le_bytes();
        let mut transaction = [Operation::Write(&mem_address), Operation::Write(buf)];

        self.i2c
            .transaction(self.address, &mut transaction)
            .await
            .map_err(|e| match e.kind() {
                ErrorKind::NoAcknowledge(_) => Error::OutOfBounds,
                _ => Error::I2c(e),
            })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error<I2cErr> {
    I2c(I2cErr),
    OutOfBounds,
}

impl<I2cErr> From<I2cErr> for Error<I2cErr> {
    fn from(value: I2cErr) -> Self {
        Self::I2c(value)
    }
}
