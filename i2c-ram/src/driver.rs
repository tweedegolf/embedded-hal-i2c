use embedded_hal_i2c::{AddressMode, AsyncI2cController, Error as _, ErrorKind};

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
        const CHUNK_SIZE: usize = 16;
        const ADDR_SIZE: usize = size_of::<u16>();

        let mut chunk_buf = [0; { ADDR_SIZE + CHUNK_SIZE }];

        for (i, chunk) in buf.chunks(CHUNK_SIZE).enumerate() {
            let chunk_address =
                u16::try_from(address as usize + i * CHUNK_SIZE).map_err(|_| Error::OutOfBounds)?;
            let data_len = chunk.len();
            let transaction_len = data_len + ADDR_SIZE;

            let (addr_buf, data_buf) = chunk_buf.split_at_mut(ADDR_SIZE);
            addr_buf.copy_from_slice(&chunk_address.to_le_bytes());
            data_buf[..data_len].copy_from_slice(chunk);

            self.i2c
                .write(self.address, &chunk_buf[..transaction_len])
                .await
                .map_err(|e| match e.kind() {
                    ErrorKind::NoAcknowledge(_) => Error::OutOfBounds,
                    _ => Error::I2c(e),
                })?
        }

        Ok(())
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
