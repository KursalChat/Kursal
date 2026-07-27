use std::io::{self, BufRead};
use tokio::sync::mpsc;
use uuid::Uuid;

use ble_peripheral_rust::{
    gatt::{
        characteristic::Characteristic,
        descriptor::Descriptor,
        peripheral_event::{
            PeripheralEvent, ReadRequestResponse, RequestResponse, WriteRequestResponse,
        },
        properties::{AttributePermission, CharacteristicProperty},
        service::Service,
    },
    uuid::ShortUuid,
    Peripheral, PeripheralImpl,
};

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "info");
    if let Err(err) = pretty_env_logger::try_init() {
        eprintln!("WARNING: failed to initialize logging framework: {}", err);
    }
    start_app().await;
}

async fn start_app() {
    let char_uuid = Uuid::from_short(0x2A3D_u16);

    // Define Service With Characteristics
    let service = Service {
        uuid: Uuid::from_short(0x1234_u16),
        primary: true,
        characteristics: vec![
            Characteristic {
                uuid: char_uuid,
                properties: vec![
                    CharacteristicProperty::Read,
                    CharacteristicProperty::Write,
                    CharacteristicProperty::Notify,
                ],
                permissions: vec![
                    AttributePermission::Readable,
                    AttributePermission::Writeable,
                ],
                value: None,
                descriptors: vec![Descriptor {
                    uuid: Uuid::from_short(0x2A13_u16),
                    value: Some(vec![0, 1]),
                    ..Default::default()
                }],
            },
            Characteristic {
                uuid: Uuid::from_string("1209"),
                ..Default::default()
            },
        ],
    };

    let (sender_tx, mut receiver_rx) = mpsc::channel::<PeripheralEvent>(256);

    let mut peripheral = Peripheral::new(sender_tx).await.unwrap();

    // Handle Updates
    tokio::spawn(async move {
        while let Some(event) = receiver_rx.recv().await {
            handle_updates(event);
        }
    });

    while !peripheral.is_powered().await.unwrap() {}

    if let Err(err) = peripheral.add_service(&service).await {
        log::error!("Error adding service: {}", err);
        return;
    }
    log::info!("Service Added");

    if let Err(err) = peripheral
        .start_advertising("RustBLE", &[service.uuid])
        .await
    {
        log::error!("Error starting advertising: {}", err);
        return;
    }
    log::info!("Advertising Started");

    // Write in console to send to characteristic update to subscribed clients
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(input) => {
                println!("Writing: {input} to {char_uuid}");
                peripheral
                    .update_characteristic(char_uuid, input.into())
                    .await
                    .unwrap();
            }
            Err(err) => {
                log::error!("Error reading from console: {}", err);
                break;
            }
        }
    }
}

/// Listen to all updates and respond if require
pub fn handle_updates(update: PeripheralEvent) {
    match update {
        PeripheralEvent::StateUpdate { is_powered } => {
            log::info!("PowerOn: {is_powered:?}")
        }
        PeripheralEvent::CharacteristicSubscriptionUpdate {
            request,
            subscribed,
        } => {
            log::info!("CharacteristicSubscriptionUpdate: Subscribed {subscribed} {request:?}")
        }
        PeripheralEvent::ReadRequest {
            request,
            offset,
            responder,
        } => {
            log::info!("ReadRequest: {request:?} Offset: {offset}");
            responder
                .send(ReadRequestResponse {
                    value: String::from("hi").into(),
                    response: RequestResponse::Success,
                })
                .unwrap();
        }
        PeripheralEvent::WriteRequest {
            request,
            offset,
            value,
            responder,
        } => {
            log::info!("WriteRequest: {request:?} Value: {value:?} Offset: {offset}");
            responder
                .send(WriteRequestResponse {
                    response: RequestResponse::Success,
                })
                .unwrap();
        }
    }
}
