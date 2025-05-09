use embedded_hal_i2c::{
    AnyAddress, AsyncI2cController, AsyncI2cTarget, AsyncReadTransaction, AsyncWriteTransaction,
    Error, ErrorKind, NoAcknowledgeSource, Operation, ReadResult, Transaction,
    TransactionExpectRead, TransactionExpectWrite, WriteResult,
};
use simulator::simulator;

#[tokio::test]
async fn test_deselect_generation() {
    let (mut c, mut t) = simulator();

    let control = async move {
        assert!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3, 4])])
                .await
                .is_ok()
        );
        let mut data = [0u8; 4];
        assert!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data)])
                .await
                .is_ok()
        );
        assert_eq!(data, [5, 6, 7, 8]);
        assert!(
            c.transaction(
                0x20u8,
                &mut [
                    Operation::Write(&[9, 10, 11, 12]),
                    Operation::Read(&mut data),
                ],
            )
            .await
            .is_ok()
        );
        assert_eq!(data, [13, 14, 15, 16]);
    };

    let target = async move {
        let Transaction::Write {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let mut data = [0u8; 4];
        assert_eq!(handler.handle_complete(&mut data).await.unwrap(), 4);
        assert_eq!(data, [1, 2, 3, 4]);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Read {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        assert_eq!(
            handler.handle_complete(&[5, 6, 7, 8], 0xff).await.unwrap(),
            4
        );
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Write {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let mut data = [0u8; 4];
        assert_eq!(handler.handle_complete(&mut data).await.unwrap(), 4);
        assert_eq!(data, [9, 10, 11, 12]);
        let Transaction::Read {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        assert_eq!(
            handler
                .handle_complete(&[13, 14, 15, 16], 0xff)
                .await
                .unwrap(),
            4
        );
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };
    };

    tokio::join!(control, target);
}

#[tokio::test]
async fn test_handle_complete() {
    let (mut c, mut t) = simulator();

    let control = async move {
        assert!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3, 4])])
                .await
                .is_ok()
        );
        assert!(matches!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3, 4, 5])])
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data)
        ));

        let mut data4 = [0u8; 4];
        assert!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data4)])
                .await
                .is_ok()
        );
        assert_eq!(data4, [1, 2, 3, 4]);
        let mut data5 = [0u8; 5];
        assert!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data5)])
                .await
                .is_ok()
        );
        assert_eq!(data5, [1, 2, 3, 4, 0xff]);
    };

    let target = async move {
        let Transaction::Write {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let mut data = [0u8; 4];
        assert_eq!(handler.handle_complete(&mut data).await.unwrap(), 4);
        assert_eq!(data, [1, 2, 3, 4]);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Write {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let mut data = [0u8; 4];
        assert_eq!(handler.handle_complete(&mut data).await.unwrap(), 4);
        assert_eq!(data, [1, 2, 3, 4]);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Read {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        assert_eq!(
            handler.handle_complete(&[1, 2, 3, 4], 0xff).await.unwrap(),
            4
        );
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Read {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        assert_eq!(
            handler.handle_complete(&[1, 2, 3, 4], 0xff).await.unwrap(),
            5
        );
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };
    };

    tokio::join!(control, target);
}

#[tokio::test]
async fn test_handle_part() {
    let (mut c, mut t) = simulator();

    let control = async move {
        assert!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3])])
                .await
                .is_ok()
        );
        assert!(matches!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3, 4])])
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data)
        ));

        let mut data4 = [0u8; 4];
        assert!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data4)])
                .await
                .is_ok()
        );
        assert_eq!(data4, [1, 2, 3, 4]);
        let mut data5 = [0u8; 5];
        assert!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data5)])
                .await
                .is_ok()
        );
        assert_eq!(data5, [1, 2, 3, 4, 42]);
    };

    let target = async move {
        let Transaction::Write {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let mut data = [0u8; 4];
        let WriteResult::Complete(3) = handler.handle_part(&mut data).await.unwrap() else {
            panic!("Unexpected write result");
        };
        assert_eq!(data, [1, 2, 3, 0]);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Write {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let mut data = [0u8; 4];
        let WriteResult::Partial(_) = handler.handle_part(&mut data).await.unwrap() else {
            panic!("Unexpected write result");
        };
        assert_eq!(data, [1, 2, 3, 4]);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Read {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let ReadResult::Complete(4) = handler.handle_part(&[1, 2, 3, 4]).await.unwrap() else {
            panic!("Unexpected read result");
        };
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Read {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let ReadResult::Partial(_) = handler.handle_part(&[1, 2, 3, 4]).await.unwrap() else {
            panic!("Unexpected read result");
        };
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };
    };

    tokio::join!(control, target);
}

#[tokio::test]
async fn test_address_nack() {
    let (mut c, mut t) = simulator();

    let control = async move {
        assert!(matches!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3, 4])])
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
        ));
        assert!(matches!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut [0, 0, 0, 0])])
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
        ));
        assert!(matches!(
            c.transaction(
                0x20u8,
                &mut [
                    Operation::Write(&[1, 2, 3, 4]),
                    Operation::Write(&[1, 2, 3, 4])
                ]
            )
            .await
            .unwrap_err()
            .kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
        ));
    };

    let target = async move {
        let Transaction::Write {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        drop(handler);
        // handle spurious (but allowed!) deselect
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };
        let Transaction::Read {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        drop(handler);
        // handle spurious (but allowed!) deselect
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Write {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let mut data = [0u8; 4];
        assert_eq!(handler.handle_complete(&mut data).await.unwrap(), 4);
        assert_eq!(data, [1, 2, 3, 4]);
        let Transaction::Write {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        drop(handler);
        // Note: this deselect is required!
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };
    };

    tokio::join!(control, target);
}

#[tokio::test]
async fn handle_part_edgecases() {
    let (mut c, mut t) = simulator();

    let control = async move {
        assert!(matches!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3, 4])])
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
        ));
        assert!(matches!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3, 4])])
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data)
        ));

        let mut data = [0u8; 4];
        assert!(matches!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data)])
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
        ));
        assert_eq!(data, [0; 4]);

        let mut data = [0u8; 4];
        assert!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data)])
                .await
                .is_ok()
        );
        assert_eq!(data, [1, 2, 3, 42]);
    };

    let target = async move {
        let Transaction::Write {
            address: AnyAddress::Seven(0x20u8),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let WriteResult::Partial(handler) = handler.handle_part(&mut []).await.unwrap() else {
            panic!("Unexpected write result");
        };
        drop(handler);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Write {
            address: AnyAddress::Seven(0x20u8),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let mut data = [0u8; 4];
        let WriteResult::Partial(handler) = handler.handle_part(&mut data).await.unwrap() else {
            panic!("Unexpected write result");
        };
        assert_eq!(data, [1, 2, 3, 4]);
        let WriteResult::Partial(handler) = handler.handle_part(&mut []).await.unwrap() else {
            panic!("Unexpected write result");
        };
        drop(handler);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Read {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let ReadResult::Partial(handler) = handler.handle_part(&[]).await.unwrap() else {
            panic!("Unexpected read result");
        };
        drop(handler);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Read {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let ReadResult::Partial(handler) = handler.handle_part(&[1, 2, 3]).await.unwrap() else {
            panic!("Unexpected read result");
        };
        let ReadResult::Partial(handler) = handler.handle_part(&[]).await.unwrap() else {
            panic!("Unexpected read result");
        };
        drop(handler);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };
    };

    tokio::join!(control, target);
}

#[tokio::test]
async fn handle_complete_edgecases() {
    let (mut c, mut t) = simulator();

    let control = async move {
        assert!(matches!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3, 4])])
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Data)
        ));
        assert!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3, 4])])
                .await
                .is_ok()
        );

        let mut data = [0u8; 4];
        assert!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data)])
                .await
                .is_ok()
        );
        assert_eq!(data, [0xff; 4]);

        let mut data = [0u8; 4];
        assert!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data)])
                .await
                .is_ok()
        );
        assert_eq!(data, [1, 2, 3, 0xff]);
    };

    let target = async move {
        let Transaction::Write {
            address: AnyAddress::Seven(0x20u8),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        assert_eq!(handler.handle_complete(&mut []).await.unwrap(), 0);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Write {
            address: AnyAddress::Seven(0x20u8),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let mut data = [0u8; 4];
        let WriteResult::Partial(handler) = handler.handle_part(&mut data).await.unwrap() else {
            panic!("Unexpected write result");
        };
        assert_eq!(data, [1, 2, 3, 4]);
        assert_eq!(handler.handle_complete(&mut []).await.unwrap(), 0);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Read {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        assert_eq!(handler.handle_complete(&[], 0xff).await.unwrap(), 4);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let Transaction::Read {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t.listen().await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let ReadResult::Partial(handler) = handler.handle_part(&[1, 2, 3]).await.unwrap() else {
            panic!("Unexpected read result");
        };
        assert_eq!(handler.handle_complete(&[], 0xff).await.unwrap(), 1);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };
    };

    tokio::join!(control, target);
}

#[tokio::test]
async fn listen_expect_matches() {
    let (mut c, mut t) = simulator();

    let control = async move {
        assert!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3, 4])])
                .await
                .is_ok()
        );
        assert!(
            c.transaction(0x20u8, &mut [Operation::Write(&[5, 6, 7])])
                .await
                .is_ok()
        );

        let mut data = [0u8; 4];
        assert!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data)])
                .await
                .is_ok()
        );
        assert_eq!(data, [8, 9, 10, 11]);
        let mut data = [0u8; 5];
        assert!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data)])
                .await
                .is_ok()
        );
        assert_eq!(data, [12, 13, 14, 15, 16]);
    };

    let target = async move {
        let mut data = [0u8; 4];
        let TransactionExpectWrite::ExpectedPartialWrite { handler } = t
            .listen_expect_write(0x20u8.into(), &mut data)
            .await
            .unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        assert_eq!(handler.handle_complete(&mut []).await.unwrap(), 0);
        assert_eq!(data, [1, 2, 3, 4]);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let mut data = [0u8; 4];
        let TransactionExpectWrite::ExpectedCompleteWrite { size: 3 } = t
            .listen_expect_write(0x20u8.into(), &mut data)
            .await
            .unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        assert_eq!(data, [5, 6, 7, 0]);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let TransactionExpectRead::ExpectedCompleteRead { size: 4 } = t
            .listen_expect_read(0x20u8.into(), &[8, 9, 10, 11])
            .await
            .unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let TransactionExpectRead::ExpectedPartialRead { handler } = t
            .listen_expect_read(0x20u8.into(), &[12, 13, 14, 15])
            .await
            .unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        assert_eq!(handler.handle_complete(&[16], 0xff).await.unwrap(), 1);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };
    };

    tokio::join!(control, target);
}

#[tokio::test]
async fn listen_expect_mismatch() {
    let (mut c, mut t) = simulator();

    let control = async move {
        assert!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3, 4])])
                .await
                .is_ok()
        );

        let mut data = [0u8; 4];
        assert!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data)])
                .await
                .is_ok()
        );
        assert_eq!(data, [5, 6, 7, 8]);
    };

    let target = async move {
        let TransactionExpectRead::Write {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t
            .listen_expect_read(0x20u8.into(), &[9, 10, 11, 12])
            .await
            .unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        let mut data = [0u8; 4];
        assert_eq!(handler.handle_complete(&mut data).await.unwrap(), 4);
        assert_eq!(data, [1, 2, 3, 4]);
        let TransactionExpectRead::Deselect = t
            .listen_expect_read(0x20u8.into(), &[13, 14, 15, 16])
            .await
            .unwrap()
        else {
            panic!("Unexpected transaction type");
        };

        let mut data = [0u8; 4];
        let TransactionExpectWrite::Read {
            address: AnyAddress::Seven(0x20),
            handler,
        } = t
            .listen_expect_write(0x20u8.into(), &mut data)
            .await
            .unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        assert_eq!(
            handler.handle_complete(&[5, 6, 7, 8], 0xff).await.unwrap(),
            4
        );
        assert_eq!(data, [0; 4]);
        let TransactionExpectWrite::Deselect = t
            .listen_expect_write(0x20u8.into(), &mut data)
            .await
            .unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        assert_eq!(data, [0; 4]);
    };

    tokio::join!(control, target);
}

#[tokio::test]
async fn listen_expect_edgecases() {
    let (mut c, mut t) = simulator();

    let control = async move {
        assert!(matches!(
            c.transaction(0x20u8, &mut [Operation::Write(&[1, 2, 3, 4])])
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
        ));

        let mut data = [0u8; 4];
        assert!(matches!(
            c.transaction(0x20u8, &mut [Operation::Read(&mut data)])
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address)
        ));
        assert_eq!(data, [0; 4]);
    };

    let target = async move {
        let TransactionExpectWrite::ExpectedPartialWrite { handler } =
            t.listen_expect_write(0x20u8.into(), &mut []).await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        drop(handler);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };

        let TransactionExpectRead::ExpectedPartialRead { handler } =
            t.listen_expect_read(0x20u8.into(), &[]).await.unwrap()
        else {
            panic!("Unexpected transaction type");
        };
        drop(handler);
        let Transaction::Deselect = t.listen().await.unwrap() else {
            panic!("Unexpected transaction type");
        };
    };

    tokio::join!(control, target);
}
