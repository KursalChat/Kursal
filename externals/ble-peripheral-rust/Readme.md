### Ble Peripheral Rust

[![Crates.io Version](https://img.shields.io/crates/v/ble-peripheral-rust)](https://crates.io/crates/ble-peripheral-rust)
[![docs.rs page](https://docs.rs/ble-peripheral-rust/badge.svg)](https://docs.rs/ble-peripheral-rust)
[![Crates.io Downloads](https://img.shields.io/crates/d/ble-peripheral-rust)](https://crates.io/crates/ble-peripheral-rust)
[![Crates.io License](https://img.shields.io/crates/l/ble-peripheral-rust)](https://crates.io/crates/ble-peripheral-rust)

BlePeripheralRust is a cross-platform Rust crate that allows your device to function as a Bluetooth Low Energy (BLE) peripheral, enables you to define BLE services, characteristics, and handle BLE events asynchronously.

## Getting Started

Check out the [examples](./examples/) folder for detailed usage, or start by running:

```sh
cargo run --example server
```

## Usage

### Initialize the Peripheral

To initialize the peripheral, create a channel to handle events, instantiate the peripheral, and wait until the BLE device is powered on:

```rust
let (sender_tx, mut receiver_rx) = channel::<PeripheralEvent>(256);
let mut peripheral = Peripheral::new(sender_tx).await.unwrap();

// Ensure the peripheral is powered on
while !peripheral.is_powered().await.unwrap() {}
```

### Add Services

Define and add a BLE service, including characteristics, descriptors with specified properties and permissions:

```rust
peripheral.add_service(
    &Service {
        uuid: Uuid::from_short(0x1234_u16),
        primary: true,
        characteristics: vec![
            Characteristic {
                uuid: Uuid::from_short(0x2A3D_u16),
                ..Default::default()
            }
        ],
    }
).await;
```

### Start Advertising

Begin advertising the BLE peripheral to make it discoverable by other devices:

```rust
peripheral.start_advertising("RustBLE", &[Uuid::from_short(0x1234_u16)]).await;
```

### Handle Events

Manage BLE events such as characteristic subscription updates, read requests, and write requests in an asynchronous loop:

```rust
while let Some(event) = receiver_rx.recv().await {
    match event {
        PeripheralEvent::CharacteristicSubscriptionUpdate { request, subscribed } => {
            // Send notifications to subscribed clients
        }
        PeripheralEvent::ReadRequest { request, offset, responder } => {
            // Respond to Read request
            responder.send(ReadRequestResponse {
                value: String::from("Hello").into(),
                response: RequestResponse::Success,
            });
        }
        PeripheralEvent::WriteRequest { request, offset, value, responder } => {
            // Respond to Write request
            responder.send(WriteRequestResponse {
                response: RequestResponse::Success,
            });
        },
        - => {}
    }
}
```

### Update Characteristics

Send characteristic updates to all clients listening to the characteristic:

```rust
peripheral.update_characteristic(Uuid::from_short(0x2A3D_u16), "Ping!".into()).await;
```

## Notes

This crate is inspired by [bluster](https://github.com/dfrankland/bluster). Contributions, bug reports, and feature requests are welcome!
