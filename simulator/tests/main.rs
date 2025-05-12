use embedded_hal_i2c::{
    AnyAddress, AsyncI2cController, AsyncI2cTarget, AsyncReadTransaction, AsyncWriteTransaction,
    ErrorKind, NoAcknowledgeSource, Operation, ReadResult, Transaction, WriteResult,
};
use simulator::simulator;

const A7: u8 = 0x42;
const ADDR: AnyAddress = AnyAddress::Seven(A7);

#[tokio::test]
async fn write_read() {
    let (mut c, mut t) = simulator();

    let control = async move {
        let mut response = [0; 8];
        c.write_read(A7, &[1, 2, 3, 4], &mut response)
            .await
            .unwrap();

        assert_eq!(response, [1, 2, 3, 4, 5, 6, 7, 8]);
    };

    let target = async move {
        let Transaction::Write { address, handler } = t.listen().await.unwrap() else {
            panic!()
        };

        assert_eq!(address, ADDR);
        let mut buffer = [0; 4];
        let written = handler.handle_complete(&mut buffer).await.unwrap();
        assert_eq!(written, 4);
        assert_eq!(buffer, [1, 2, 3, 4]);

        let Transaction::Read { address, handler } = t.listen().await.unwrap() else {
            panic!()
        };
        assert_eq!(address, ADDR);
        let buffer = [1, 2, 3, 4, 5, 6, 7, 8];
        handler.handle_complete(&buffer, 0xFF).await.unwrap();

        assert!(matches!(t.listen().await.unwrap(), Transaction::Deselect));

        t
    };

    tokio::join!(control, target);
}

#[tokio::test]
async fn nacking_everything() {
    let (mut c, mut t) = simulator();

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
        let Transaction::Read { address, handler } = t.listen().await.unwrap() else {
            panic!()
        };
        assert_eq!(address, ADDR);
        drop(handler);

        assert!(matches!(t.listen().await.unwrap(), Transaction::Deselect));

        let Transaction::Write { address, handler } = t.listen().await.unwrap() else {
            panic!()
        };
        assert_eq!(address, ADDR);
        drop(handler);

        assert!(matches!(t.listen().await.unwrap(), Transaction::Deselect));

        let Transaction::Write { address, handler } = t.listen().await.unwrap() else {
            panic!()
        };
        assert_eq!(address, ADDR);
        handler.handle_complete(&mut [0]).await.unwrap();

        assert!(matches!(t.listen().await.unwrap(), Transaction::Deselect));

        // Only drop once we are done
        t
    };

    let out = tokio::join!(control, target);
    drop(out);
}

#[tokio::test]
async fn long_transation() {
    let (mut c, mut t) = simulator();

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
            let Transaction::Write { address, handler } = t.listen().await.unwrap() else {
                panic!()
            };
            assert_eq!(address, ADDR);
            let mut buf = [0];
            let len = handler.handle_complete(&mut buf).await.unwrap();
            assert_eq!(&buf[..len], [expect]);
        }

        for expect in [3, 4] {
            let Transaction::Read { address, handler } = t.listen().await.unwrap() else {
                panic!()
            };
            assert_eq!(address, ADDR);
            let ReadResult::Complete(len) = handler.handle_part(&[expect, 0]).await.unwrap() else {
                panic!()
            };
            assert_eq!(len, 1);
        }

        for expect in [5, 6] {
            let Transaction::Write { address, handler } = t.listen().await.unwrap() else {
                panic!()
            };
            assert_eq!(address, ADDR);
            let mut buf = [0];
            let len = handler.handle_complete(&mut buf).await.unwrap();
            assert_eq!(&buf[..len], [expect]);
        }

        assert!(matches!(t.listen().await.unwrap(), Transaction::Deselect));
    };

    tokio::join!(control, target);
}

#[tokio::test]
async fn write_nak() {
    let (mut c, mut t) = simulator();

    let control = async move {
        c.write(A7, &[0, 0]).await.unwrap();
    };

    let target = async move {
        let Transaction::Write { handler, .. } = t.listen().await.unwrap() else {
            panic!()
        };
        let WriteResult::Partial(handler) = handler.handle_part(&mut [0, 0]).await.unwrap() else {
            panic!("unexpected complete")
        };

        match handler.handle_part(&mut [0]).await.unwrap() {
            WriteResult::Complete(0) => {}
            WriteResult::Complete(cnt) => {
                panic!("too long complete: {cnt}")
            }
            WriteResult::Partial(h) => {
                drop(h);
                panic!("Unexpected partial")
            }
        }

        assert!(matches!(t.listen().await.unwrap(), Transaction::Deselect));
    };

    tokio::join!(control, target);
}
