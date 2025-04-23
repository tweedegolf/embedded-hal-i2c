use embedded_hal_i2c_target::{ExpectHandledWrite, I2cTarget};
use std::sync::atomic::{AtomicBool, Ordering};

pub mod tests;

pub trait Interface {
    type Error;

    fn read_reg<'buf>(&mut self, addr: u8, buf: &'buf mut [u8]) -> Result<&'buf [u8], Self::Error>;
    fn write_reg(&mut self, addr: u8, data: &[u8]) -> Result<(), Self::Error>;
}

pub async fn run(mut i2c: impl I2cTarget, mut interface: impl Interface, stop: &AtomicBool) {
    let my_address = 0x2a;

    let mut buf = [0u8; 64];
    while !stop.load(Ordering::Relaxed) {
        // We need to start with a write. This will either be a single byte (for a "write then read"),
        // or a multi-byte sequence (for a "write then write")
        let res = i2c.listen_expect_write(my_address, &mut buf).await;
        let Ok(ExpectHandledWrite::HandledCompletely(size)) = res else {
            // I dunno what they wanted.
            continue;
        };
        drop(res);

        let used = &buf[..size];
        match used {
            [] => {
                // why do you send me this empty write transaction
                continue;
            }
            [reg_addr] => {
                // We were written just an address, prep for a switch to a read
                if let Ok(data) = interface.read_reg(*reg_addr, &mut buf) {
                    // we don't really care if they gave up, if this is complete, then great,
                    // if not, we'll drop the handler
                    if let Ok(t) = i2c.listen_expect_read(my_address, data).await {
                        t.done().await
                    }
                }
            }
            [reg_addr, data @ ..] => {
                let _ = interface.write_reg(*reg_addr, data);
            }
        }
    }
}
