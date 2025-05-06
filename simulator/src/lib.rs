#![warn(missing_docs)]

//! This crate provides an implementation of [`I2cTarget`] that can be run locally.
//!
//! # Example
//! ```rust
//! use embedded_hal_i2c::AnyAddress;
//! use simulator::simulator;
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() {
//! use embedded_hal_i2c::{
//!     AsyncI2cController, I2cTarget, ReadTransaction, Transaction, WriteTransaction,
//! };
//! use std::time::Duration;
//! let (mut controller, mut target) = simulator(AnyAddress::Seven(42));
//!
//! let controller_task = async move {
//!     let mut response = [0; 5];
//!     controller
//!         .write_read(42_u8, &0xdeadbeef_u32.to_be_bytes(), &mut response)
//!         .await
//!         .unwrap();
//!     assert_eq!(response, [0xc0, 0xff, 0xee, 0x00, 0xff]);
//! };
//!
//! let target_task = async move {
//!     let Ok(Transaction::WriteTransaction { address, handler }) = target.listen().await else {
//!         unreachable!()
//!     };
//!     assert_eq!(address, AnyAddress::Seven(42));
//!     let mut data = [0; 4];
//!     let len = handler.handle_complete(&mut data).await.unwrap();
//!     assert_eq!(&data[..len], &0xdeadbeef_u32.to_be_bytes());
//!
//!     let Ok(Transaction::ReadTransaction { address, handler }) = target.listen().await else {
//!         unreachable!()
//!     };
//!     let response = 0xc0ffee00_u32.to_be_bytes();
//!     assert_eq!(address, AnyAddress::Seven(42));
//!     handler.handle_complete(&response, 0xff).await.unwrap();
//! };
//!
//! # tokio::time::timeout(Duration::from_secs(1), async move {
//! tokio::join!(controller_task, target_task);
//! # }).await.unwrap();
//! # }
//! ```

use controller::SimController;
use embedded_hal_i2c::{AnyAddress, ErrorKind};
use target::SimTarget;
use tokio::sync::mpsc::channel;
use tokio::sync::oneshot;

#[cfg(doc)]
use embedded_hal_i2c::I2cTarget;

pub mod controller;
pub mod target;

/// Create an I2C controller and target pair
///
/// The returned [`SimController`] implements the `embedded-hal` trait for I2C.
/// And the [`SimTarget`] implements the new target traits from `embedded-hal-i2c`.
pub fn simulator(address: AnyAddress) -> (SimController, SimTarget) {
    let (to_target, from_controller) = channel(1);

    (
        SimController::new(to_target),
        SimTarget::new(address, from_controller),
    )
}

#[derive(Debug, PartialEq, Eq)]
enum SimOp {
    Read(Vec<u8>),
    Write(Vec<u8>),
}

struct SimTransaction {
    address: AnyAddress,
    actions: Vec<SimOp>,
}

struct PartialTransaction {
    transaction: SimTransaction,
    current_op: usize,
    responder: oneshot::Sender<Result<SimTransaction, ErrorKind>>,
}

impl PartialTransaction {
    const fn new(
        transaction: SimTransaction,
        responder: oneshot::Sender<Result<SimTransaction, ErrorKind>>,
    ) -> Self {
        Self {
            transaction,
            current_op: 0,
            responder,
        }
    }

    fn current(&self) -> Option<&SimOp> {
        self.transaction.actions.get(self.current_op)
    }
    fn current_mut(&mut self) -> Option<&mut SimOp> {
        self.transaction.actions.get_mut(self.current_op)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_i2c::{
        AsyncI2cController, I2cTarget, NoAcknowledgeSource, Operation, ReadResult, ReadTransaction,
        Transaction, WriteTransaction,
    };

    const A7: u8 = 0x42;
    const ADDR: AnyAddress = AnyAddress::Seven(A7);

    #[tokio::test]
    async fn write_read() {
        let (mut c, mut t) = simulator(ADDR);

        let control = async move {
            let mut response = [0; 8];
            c.write_read(A7, &[1, 2, 3, 4], &mut response)
                .await
                .unwrap();

            assert_eq!(response, [1, 2, 3, 4, 5, 6, 7, 8]);
        };

        let target = async move {
            let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };

            assert_eq!(address, ADDR);
            let mut buffer = [0; 4];
            let written = handler.handle_complete(&mut buffer).await.unwrap();
            assert_eq!(written, 4);
            assert_eq!(buffer, [1, 2, 3, 4]);

            let Transaction::ReadTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(address, ADDR);
            let buffer = [1, 2, 3, 4, 5, 6, 7, 8];
            handler.handle_complete(&buffer, 0xFF).await.unwrap();
        };

        tokio::join!(control, target);
    }

    #[tokio::test]
    async fn nacking_everything() {
        let (mut c, mut t) = simulator(ADDR);

        let control = async move {
            let result = c.read(A7, &mut []).await.unwrap_err();
            assert_eq!(
                result,
                ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
            );

            let result = c.write(A7, &[]).await.unwrap_err();
            assert_eq!(
                result,
                ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
            );

            let result = c.write(A7, &[1, 2, 3]).await.unwrap_err();
            assert_eq!(result, ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data));
        };

        let target = async move {
            let Transaction::ReadTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(address, ADDR);
            drop(handler);

            let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(address, ADDR);
            drop(handler);

            let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(address, ADDR);
            handler.handle_complete(&mut [0]).await.unwrap();

            // Only drop once we are done
            t
        };

        tokio::join!(control, target);
    }

    #[tokio::test]
    async fn long_transation() {
        let (mut c, mut t) = simulator(ADDR);

        let control = async move {
            let mut a = [0];
            let mut b = [0];
            let mut transactions = [
                Operation::Write(&[1]),
                Operation::Write(&[2]),
                Operation::Read(&mut a),
                Operation::Read(&mut b),
                Operation::Write(&[5]),
                Operation::Write(&[6]),
            ];

            c.transaction(A7, &mut transactions).await.unwrap();

            assert_eq!(a, [3]);
            assert_eq!(b, [4]);
        };

        let target = async move {
            for expect in [1, 2] {
                let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
                else {
                    panic!()
                };
                assert_eq!(address, ADDR);
                let mut buf = [0];
                let len = handler.handle_complete(&mut buf).await.unwrap();
                assert_eq!(&buf[..len], [expect]);
            }

            for expect in [3, 4] {
                let Transaction::ReadTransaction { address, handler } = t.listen().await.unwrap()
                else {
                    panic!()
                };
                assert_eq!(address, ADDR);
                let ReadResult::Finished(len) = handler.handle_part(&[expect, 0]).await.unwrap()
                else {
                    panic!()
                };
                assert_eq!(len, 1);
            }

            for expect in [5, 6] {
                let Transaction::WriteTransaction { address, handler } = t.listen().await.unwrap()
                else {
                    panic!()
                };
                assert_eq!(address, ADDR);
                let mut buf = [0];
                let len = handler.handle_complete(&mut buf).await.unwrap();
                assert_eq!(&buf[..len], [expect]);
            }
        };

        tokio::join!(control, target);
    }
}
