use crate::Interface;
use embedded_hal_i2c::I2cTarget;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

struct TestInterface {
    data: [u32; 32],
}

impl Interface for TestInterface {
    type Error = ();

    fn read_reg<'buf>(&mut self, addr: u8, buf: &'buf mut [u8]) -> Result<&'buf [u8], Self::Error> {
        if buf.len() < 4 {
            return Err(());
        }

        let data = self.data[usize::from(addr)];
        buf[..4].copy_from_slice(&data.to_le_bytes());

        Ok(&buf[..4])
    }

    fn write_reg(&mut self, addr: u8, data: &[u8]) -> Result<(), Self::Error> {
        let Ok(data) = data.try_into() else {
            return Err(());
        };

        let word = u32::from_le_bytes(data);
        self.data[usize::from(addr)] = word;

        Ok(())
    }
}

pub async fn server(i2c: impl I2cTarget, stop: Arc<AtomicBool>) {
    let iface = TestInterface { data: [0; 32] };
    super::run(i2c, iface, &stop).await;
}

#[cfg(test)]
mod test_locally {
    use super::*;
    use embedded_hal_i2c::AsyncI2cController;
    use std::sync::atomic::Ordering;
    use tokio::join;

    #[tokio::test]
    async fn works_locally() {
        let (mut cont, target) = simulator::simulator(0x2a_u8);

        let stop = Arc::new(AtomicBool::new(false));
        let server_fut = server(target, Arc::clone(&stop));

        let client_fut = async move {
            for i in 0..32 {
                let mut buf = [0xFF; 4];
                cont.write_read(0x2a, &[i], &mut buf).await.unwrap();

                assert_eq!(buf, [0; 4]);
            }

            for i in 0..32 {
                let buf = [i, i, 0, 0, 0];
                cont.write(0x2a, &buf).await.unwrap();
            }

            for i in 0..32 {
                let mut buf = [0xFF; 4];
                cont.write_read(0x2a, &[i], &mut buf).await.unwrap();

                assert_eq!(buf, [i, 0, 0, 0]);
            }

            stop.store(true, Ordering::Relaxed);
        };

        join!(server_fut, client_fut);
    }

    #[tokio::test]
    async fn too_short_is_ignored() {
        let (mut cont, target) = simulator::simulator(0x2a_u8);

        let stop = Arc::new(AtomicBool::new(false));
        let server_fut = server(target, Arc::clone(&stop));

        let client_fut = async move {
            let buf = [0, 1, 2, 3];
            cont.write(0x2a, &buf).await.unwrap();

            for i in 0..32 {
                let mut buf = [0xFF; 4];
                cont.write_read(0x2a, &[i], &mut buf).await.unwrap();

                assert_eq!(buf, [0, 0, 0, 0]);
            }

            stop.store(true, Ordering::Relaxed);
        };

        join!(server_fut, client_fut);
    }

    #[tokio::test]
    async fn overreading_is_filled() {
        let (mut cont, target) = simulator::simulator(0x2a_u8);

        let stop = Arc::new(AtomicBool::new(false));
        let server_fut = server(target, Arc::clone(&stop));

        let client_fut = async move {
            let mut buf = [0xFF; 5];
            cont.write_read(0x2a, &[0], &mut buf).await.unwrap();

            assert_eq!(buf, [0, 0, 0, 0, 42]);

            stop.store(true, Ordering::Relaxed);
        };

        join!(server_fut, client_fut);
    }
}
