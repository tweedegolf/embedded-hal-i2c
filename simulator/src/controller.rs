//! Controller half implementation of the simulator

use crate::{PartialTransaction, SimOp, SimTransaction};
use embedded_hal_i2c::{
    AddressMode, AnyAddress, AsyncI2cController, ErrorKind, ErrorType, Operation,
};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

/// Simulated I2C controller
///
/// This can be created with [`crate::simulator`], which also returns the linked [`SimTarget`].
/// All [`transaction`] calls on this controller are forwarded to the target
/// as if there was a real I2C bus connecting the two.
pub struct SimController {
    to_target: Sender<PartialTransaction>,
}

impl SimController {
    pub(crate) const fn new(to_target: Sender<PartialTransaction>) -> Self {
        Self { to_target }
    }
}

impl ErrorType for SimController {
    type Error = ErrorKind;
}

impl<A> AsyncI2cController<A> for SimController
where
    A: AddressMode + Into<AnyAddress>,
{
    async fn transaction(
        &mut self,
        address: A,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        let address = address.into();
        let actions = operations
            .iter()
            .map(|a| match a {
                Operation::Read(r) => SimOp::Read(vec![0; r.len()]),
                Operation::Write(w) => SimOp::Write(w.to_vec()),
            })
            .collect();

        let transaction = SimTransaction { address, actions };
        let (sender, receiver) = oneshot::channel();

        self.to_target
            .try_send(PartialTransaction::new(transaction, sender))
            .unwrap();

        let response = receiver.await.map_err(|_| ErrorKind::Other)?;
        let actions = response?.actions;
        for (op, reply) in operations.iter_mut().zip(actions) {
            match (op, reply) {
                (Operation::Read(buf), SimOp::Read(response)) => {
                    assert_eq!(buf.len(), response.len());
                    buf.copy_from_slice(&response[..]);
                }
                (Operation::Write(_), SimOp::Write(_)) => {}
                _ => panic!("send operation does not matched received operation"),
            }
        }

        Ok(())
    }
}
