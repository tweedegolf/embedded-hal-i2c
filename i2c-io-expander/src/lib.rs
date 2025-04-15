use embedded_hal_i2c_target::{ExpectHandledWrite, I2cTarget};

struct Interface;
impl Interface {
    fn read_reg(&mut self, addr: u8, buf: &mut [u8]) -> Result<&[u8], ()> {
        todo!()
    }

    fn write_reg(&mut self, addr: u8, data: &[u8]) -> Result<(), ()> {
        todo!()
    }
}

async fn run(mut i2c: impl I2cTarget) {
    let my_address = 0x2a;

    // this is the interface with the rest of our program, where users can
    // set/get data or get notified on reads/writes of registers
    let mut interface = Interface;
    let mut buf = [0u8; 64];
    loop {
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
                    let _ = i2c.listen_expect_read(my_address, data).await;
                }
            }
            [reg_addr, data @ ..] => {
                let _ = interface.write_reg(*reg_addr, data);
            }
        }
    }
}
