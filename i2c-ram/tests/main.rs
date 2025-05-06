use embedded_hal_i2c::{AnyAddress, SevenBitAddress};
use i2c_ram::driver::Error::OutOfBounds;
use i2c_ram::driver::I2cRam;
use i2c_ram::{TARGET_ADDR, target_service};
use simulator::controller::SimController;
use simulator::simulator;
use std::sync::atomic::{AtomicBool, Ordering};

async fn run_with(test: impl AsyncFnOnce(I2cRam<SimController, SevenBitAddress>)) {
    let _ = env_logger::try_init();
    let (c, t) = simulator();
    let stop = AtomicBool::new(false);

    let client = async {
        let Some(AnyAddress::Seven(addr)) = TARGET_ADDR else {
            panic!("Target Address wrong")
        };

        let ram = I2cRam::new(c, addr);
        test(ram).await;
        stop.store(true, Ordering::Relaxed);
    };

    tokio::join!(client, target_service(t, &stop));
}

#[tokio::test]
async fn basic_rw() {
    run_with(async |mut ram| {
        let mut buf = [0; 513];

        ram.read(0, &mut buf).await.unwrap();
        assert_eq!(&buf[..512], &[0; 512]);
        assert_eq!(&buf[512..], &[0xFF]);

        let err = ram.read(513, &mut buf).await.unwrap_err();
        assert_eq!(err, OutOfBounds);

        let data: [u8; 8] = std::array::from_fn(|n| n as u8);
        ram.write(0, &data[..]).await.unwrap();

        let mut buf = [0; 16];
        ram.read(0, &mut buf).await.unwrap();
        assert_eq!(&buf[..8], &data[..]);
        assert_eq!(&buf[8..], &[0; 16][8..]);
    })
    .await;
}
