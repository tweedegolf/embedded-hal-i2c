# `embedded-hal-i2c`:

> Abstracting I2C Target Implementations for `embedded-hal`

The `embedded-hal` project provides a foundational set of hardware abstraction layer (HAL) traits for embedded Rust
development, enabling portable drivers and applications. While `embedded-hal` offers robust abstractions for I2C
controller (formerly master) peripherals, the **target** abstraction has been missing: a standardized way to define I2C
target peripherals (formerly slaves).

This crate, `embedded-hal-i2c`, proposes a new trait design to fill this gap. By introducing `AsyncI2cTarget` and
`SyncI2cTarget`, we aim to provide a generic interface for implementing and interacting with I2C peripheral
functionality. This allows hardware abstraction layers to expose their I2C target capabilities in a standardized way,
enables one microcontroller to act as an I2C peripheral to another, and facilitates emulating existing I2C devices as a
driver.

This crate serves as a proof-of-concept and a proposal for a future extension to the official `embedded-hal` project. We
encourage feedback and collaboration to refine this design and ultimately integrate it into `embedded-hal`.

## Design

The design for this new trait was informed by a thorough review of existing HAL implementations. For more details, you
can see [research.md](./research.md). The proposed trait prioritizes flexibility for driver implementations over
interface simplicity.

The core of this trait revolves around a single `listen()` function, which acts as the primary interface for your I2C
target device. This function waits for and returns the next action the I2C controller requests to perform, essentially
providing an event-driven model for target communication.

### The `listen()` Method and `Transaction` Enum

When you call `listen()`, it yields a **`Transaction`** enum. This enum captures the type of event or request that
occurred on the I2C bus:

- **`Transaction::Deselect`**: This variant signals that a stop or restart condition occurred on the bus, or that a
  different device has been selected.
- **`Transaction::Read` and `Transaction::Write`**: These two variants are quite similar in structure and control. Both
  indicate that the I2C controller has initiated a transaction (either a read from your device or a write to it). For
  both, you'll receive:
    - `address`: Which address the transaction is targeting.
    - `handler`: A handler object the driver uses to respond to the transaction.

### Handling Transactions: `ReadTransaction` and `WriteTransaction`

The `handler` objects for `Read` and `Write` transactions implement the `ReadTransaction` and `WriteTransaction` traits,
respectively. While `ReadTransaction` is used by the target to **send data to** the I2C controller, and
`WriteTransaction` is used to **receive data from** the I2C controller, they both offer two primary methods to respond
and a specific `drop()`/NACK behavior:

- **`handle_part()`**: Process part of the transaction. This method returns whether the transaction was completed with
  this call (and how many bytes have been successfully read/written) or provides a new handler to continue reacting to
  this transaction.
- **`handle_complete()`**: Use exactly the buffer provided and return how many bytes have been read/written. For a read,
  this will use an overrun character to pad out bytes requested after the buffer is used up. For a write, it will NACK
  any bytes which do not fit.
- **`drop()`**:
    - **Before any interaction**: NACK the address. This is typically done if the `address` does not match or the device
      is busy.
    - **On a write**: NACK all unhandled data bytes.
    - **On a read**: Respond with an implementation-defined fill byte. (A target has no way to NACK a byte being read.)

## How to implement a HAL

For examples of HAL implementations see:

* [embassy-npcx](https://github.com/OpenDevicePartnership/embassy-npcx/blob/58d06fc5f19bba003938b9af83de5283ee15c21f/src/i2c.rs#L1067)
* [embassy-imxrt](https://github.com/OpenDevicePartnership/embassy-imxrt/pull/389) (WIP)

## How to implement a driver

See examples:

* [`i2c-io-expander`](https://www.google.com/search?q=./i2c-io-expander)
* [`i2c-ram`](https://www.google.com/search?q=./i2c-ram)

## Testing a driver

There is a [`simulator`](./simulator) implementation that forwards commands from an [
`impl embedded_hal::i2c::I2c`](https://docs.rs/embedded-hal/latest/embedded_hal/i2c/trait.I2c.html) to an
`impl I2cTarget`.
See [`i2c-ram/tests/main.rs`](./i2c-ram/tests/main.rs) for an example of how to use it for testing locally.

## Why a Separate Crate / Contribution to `embedded-hal`?

This crate is intended to serve as a proof-of-concept and a proposal for extending the [
`embedded-hal`](https://docs.rs/embedded-hal/latest/embedded_hal/) project.
By developing it in a separate repository first, we can iterate on the API design, collect feedback, and test the
concept without impacting the stability of `embedded-hal` itself.

Our ultimate goal is to upstream this `I2cTarget` trait (and its asynchronous counterpart) into `embedded-hal` (and
`embedded-hal-async`) once the design is mature. We believe this standardization is important for creating a robust and
interoperable ecosystem of I2C target drivers and HAL implementations in embedded Rust.

## Contributing

We welcome contributions and feedback on this project\! Whether you're interested in refining the API design,
implementing the trait for a new HAL, or simply providing your thoughts, your input is highly valued.

Here are some ways you can contribute:

* **Open an Issue**: Report issues you see with the API or the examples.
* **Provide Feedback**: Share your thoughts on the API, how it can be implemented in HALs, and used by drivers.
* **Submit a Pull Request**: Help implement the trait for more HALs, add more examples, or contribute to the
  documentation.

## License

This project is licensed under either of the following licenses, at your option:

* Apache License, Version 2.0 ([LICENSE-APACHE](https://www.google.com/search?q=./LICENSE-APACHE))
* MIT License ([LICENSE-MIT](https://www.google.com/search?q=./LICENSE-MIT))
