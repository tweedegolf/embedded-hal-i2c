use embedded_hal_i2c::{
    AnyAddress, I2cTarget, ReadTransaction, TransactionExpectEither, WriteResult, WriteTransaction,
};
use log::info;
use std::sync::atomic::{AtomicBool, Ordering};

pub mod driver;

pub const TARGET_ADDR: Option<AnyAddress> = Some(AnyAddress::Seven(0x20));
const BUFLEN: usize = 512;

pub async fn target_service<I: I2cTarget>(mut i2c: I, stop: &AtomicBool)
where
    <I as I2cTarget>::Error: std::fmt::Debug,
{
    // Implement a simple i2c RAM, demonstrating the features
    // of the new interface.

    let mut buf = [0u8; BUFLEN];
    let mut cur_addr = 0usize;

    let mut expect_read = false;

    while !stop.load(Ordering::Relaxed) {
        let mut addr = [0u8; 2];
        let result = if expect_read && cur_addr < BUFLEN {
            i2c.listen_expect_read(
                TARGET_ADDR.unwrap(),
                buf.get(cur_addr..).unwrap_or_default(),
            )
            .await
            .map(TransactionExpectEither::from)
        } else {
            i2c.listen_expect_write(TARGET_ADDR.unwrap(), &mut addr)
                .await
                .map(TransactionExpectEither::from)
        };

        let Ok(result) = result else { continue };

        use TransactionExpectEither::*;
        match result {
            Deselect => {
                expect_read = false;
                info!("Deselection detected");
            }
            Read { handler, .. } => {
                if cur_addr >= BUFLEN {
                    // No valid address, so can't facilitate a read, nack it.
                    info!("Rejected read transaction, no valid start address");
                    drop(handler);
                } else {
                    // Provide the data for the read, and then let go of the bus after.
                    let size = handler
                        .handle_complete(&buf[cur_addr..], 0xFF)
                        .await
                        .unwrap();
                    info!(
                        "Read transaction starting at addr {}, provided {} bytes",
                        cur_addr, size
                    );
                    cur_addr = cur_addr.saturating_add(size).min(BUFLEN);
                }
            }
            ExpectedCompleteRead { size } => {
                info!(
                    "Expected read transaction starting at addr {}, provided {} bytes",
                    cur_addr, size
                );
                cur_addr = cur_addr.saturating_add(size).min(BUFLEN);
            }
            ExpectedPartialRead { handler } => {
                let size = buf.get(cur_addr..).unwrap_or_default().len()
                    + handler.handle_complete(&[], 0xFF).await.unwrap();
                info!(
                    "Expected partial read transaction starting at addr {}, provided {} bytes",
                    cur_addr, size
                );
                cur_addr = cur_addr.saturating_add(size).min(BUFLEN);
            }
            Write { handler, .. } => {
                info!("Write request");
                let mut addr = [0u8; 2];
                match handler.handle_part(&mut addr).await.unwrap() {
                    WriteResult::Partial(handler) => {
                        let new_addr: usize = u16::from_le_bytes(addr).into();
                        if new_addr < BUFLEN {
                            cur_addr = new_addr;
                            info!("Received addr {}", cur_addr);
                            expect_read = true;

                            let size_written =
                                handler.handle_complete(&mut buf[cur_addr..]).await.unwrap();
                            cur_addr += size_written;
                            info!("Received write of {} bytes to ram", size_written);
                        } else {
                            // Invalid address, nack it
                            drop(handler);
                        }
                    }
                    WriteResult::Complete(size) => {
                        info!(
                            "Incomplete address write of size {} received, ignoring",
                            size
                        );
                    }
                };
            }
            ExpectedCompleteWrite { size } => {
                info!(
                    "Expected incomplete address write of size {} received, ignoring",
                    size
                );
            }
            ExpectedPartialWrite { handler } => {
                info!("Expected partial write");
                let new_addr: usize = u16::from_le_bytes(addr).into();
                if new_addr < BUFLEN {
                    cur_addr = new_addr;
                    info!("Received addr {}", cur_addr);
                    expect_read = true;

                    let size_written = handler.handle_complete(&mut buf[cur_addr..]).await.unwrap();
                    cur_addr += size_written;
                    info!("Received write of {} bytes to ram", size_written);
                } else {
                    // Invalid address, nack it
                    drop(handler);
                }
            }
        }
    }
}
