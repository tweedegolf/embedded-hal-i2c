#![warn(missing_docs)]

//! This crate provides an implementation of [`I2cTarget`] that can be run locally.
//!
//! # Example
//! ```rust
//! use embedded_hal_i2c::{
//!     AnyAddress, AsyncI2cController, I2cTarget, ReadTransaction, Transaction, WriteTransaction,
//! };
//! use simulator::simulator;
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() {
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
//! # tokio::time::timeout(std::time::Duration::from_secs(1), async move {
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
