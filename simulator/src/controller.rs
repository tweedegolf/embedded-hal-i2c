//! Controller half implementation of the simulator

#[cfg(doc)]
use crate::target::SimTarget;
use crate::{PartialTransaction, SimOp, SimTransaction};
use embedded_hal_i2c::{
    AddressMode, AnyAddress, AsyncI2cController, ErrorKind, ErrorType, Operation, SyncI2cController,
};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use tokio::sync::oneshot::Receiver;

/// Simulated I2C controller
///
/// This can be created with [`crate::simulator`], which also returns the linked [`SimTarget`].
/// All [`AsyncI2cController::transaction`] calls on this controller are forwarded to the target
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

impl SimController {
    fn send_transaction(
        &mut self,
        address: AnyAddress,
        operations: &mut [Operation],
    ) -> Receiver<Result<SimTransaction, ErrorKind>> {
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
        receiver
    }
}

impl SimTransaction {
    fn copy_to_ops(self, operations: &mut [Operation]) {
        let actions = self.actions;
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
    }
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
        self.send_transaction(address.into(), operations)
            .await
            .map_err(|_| ErrorKind::Other)??
            .copy_to_ops(operations);
        Ok(())
    }
}

impl<A> SyncI2cController<A> for SimController
where
    A: AddressMode + Into<AnyAddress>,
{
    fn transaction(
        &mut self,
        address: A,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        self.send_transaction(address.into(), operations)
            .blocking_recv()
            .map_err(|_| ErrorKind::Other)??
            .copy_to_ops(operations);
        Ok(())
    }
}
