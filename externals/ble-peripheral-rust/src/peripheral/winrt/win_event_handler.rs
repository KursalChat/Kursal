use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::gatt::peripheral_event::{
    PeripheralEvent, PeripheralRequest, ReadRequestResponse, RequestResponse, WriteRequestResponse,
};
use crate::peripheral::winrt::win_utils::{
    buffer_to_vec, device_id_from_session, to_uuid, vec_to_buffer,
};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use uuid::Uuid;
use windows::core::IInspectable;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattProtocolError, GattReadRequest, GattServiceProviderAdvertisementStatus,
    GattSubscribedClient, GattWriteRequest,
};
use windows::Devices::Radios::{Radio, RadioState};
use windows::Foundation::Collections::IVectorView;
use windows::{
    Devices::Bluetooth::GenericAttributeProfile::{
        GattLocalCharacteristic, GattReadRequestedEventArgs, GattServiceProvider,
        GattServiceProviderAdvertisementStatusChangedEventArgs, GattWriteRequestedEventArgs,
    },
    Foundation::TypedEventHandler,
};

pub struct WinEventHandler {
    sender_tx: Sender<PeripheralEvent>,
    connected_clients: Arc<RwLock<HashMap<(Uuid, Uuid), Vec<String>>>>,
}

impl WinEventHandler {
    pub fn new(sender_tx: Sender<PeripheralEvent>) -> Self {
        Self {
            sender_tx,
            connected_clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn create_radio_listener(&self) -> TypedEventHandler<Radio, IInspectable> {
        let sender_tx: Sender<PeripheralEvent> = self.sender_tx.clone();

        return TypedEventHandler::new(
            move |originator: &Option<Radio>, _: &Option<IInspectable>| {
                let Some(radio) = originator.as_ref() else {
                    return Ok(());
                };
                let is_on = radio.State()? == RadioState::On;
                futures::executor::block_on(async {
                    if let Err(err) = sender_tx
                        .send(PeripheralEvent::StateUpdate { is_powered: is_on })
                        .await
                    {
                        log::error!("Error sending delegate event: {}", err);
                    }
                });
                Ok(())
            },
        );
    }

    pub fn create_advertisement_status_handler(
        &self,
    ) -> TypedEventHandler<
        GattServiceProvider,
        GattServiceProviderAdvertisementStatusChangedEventArgs,
    > {
        TypedEventHandler::new(move |originator: &Option<GattServiceProvider>, args: &Option<GattServiceProviderAdvertisementStatusChangedEventArgs>| {
            let (Some(service), Some(event_args)) = (originator.as_ref(), args.as_ref()) else {
                return Ok(());
            };
            let status = event_args.Status()?;
            log::debug!("Advertisement Status: {:?}: Started: {:?}",
                to_uuid(&service.Service()?.Uuid()?),
                status == GattServiceProviderAdvertisementStatus::Started);
            Ok(())
        })
    }

    pub fn create_subscribe_handler(
        &self,
        service_uuid: Uuid,
    ) -> TypedEventHandler<GattLocalCharacteristic, IInspectable> {
        let connected_clients = Arc::clone(&self.connected_clients);
        let sender_tx: Sender<PeripheralEvent> = self.sender_tx.clone();

        TypedEventHandler::new(
            move |originator: &Option<GattLocalCharacteristic>, _: &Option<IInspectable>| {
                let Some(characteristic) = originator.as_ref() else {
                    return Ok(());
                };
                let characteristic_uuid = to_uuid(&characteristic.Uuid()?);

                let subscribed_clients: IVectorView<GattSubscribedClient> =
                    characteristic.SubscribedClients()?;

                let new_clients: Vec<String> = subscribed_clients
                    .into_iter()
                    .map(|client| Ok(device_id_from_session(client.Session()?)))
                    .collect::<windows::core::Result<Vec<String>>>()?;

                let Ok(mut old_clients_store) = connected_clients.write() else {
                    log::error!("Connected clients lock poisoned");
                    return Ok(());
                };
                let mut added_clients: Vec<String> = Vec::new();
                let mut removed_clients: Vec<String> = Vec::new();

                if let Some(old_clients) =
                    old_clients_store.get_mut(&(service_uuid, characteristic_uuid))
                {
                    for client in &new_clients {
                        if !old_clients.contains(client) {
                            added_clients.push(client.clone());
                        }
                    }
                    for client in old_clients.clone() {
                        if !new_clients.contains(&client) {
                            removed_clients.push(client.clone());
                        }
                    }

                    *old_clients = new_clients;
                } else {
                    old_clients_store
                        .insert((service_uuid, characteristic_uuid), new_clients.clone());
                    added_clients.extend(new_clients.clone());
                }

                // Update Newly added/removed clients
                futures::executor::block_on(async {
                    for client in added_clients {
                        if let Err(err) = sender_tx
                            .send(PeripheralEvent::CharacteristicSubscriptionUpdate {
                                request: PeripheralRequest {
                                    client,
                                    service: service_uuid,
                                    characteristic: characteristic_uuid,
                                },
                                subscribed: true,
                            })
                            .await
                        {
                            log::error!("Error sending delegate event: {}", err);
                        }
                    }

                    for client in removed_clients {
                        if let Err(err) = sender_tx
                            .send(PeripheralEvent::CharacteristicSubscriptionUpdate {
                                request: PeripheralRequest {
                                    client,
                                    service: service_uuid,
                                    characteristic: characteristic_uuid,
                                },
                                subscribed: false,
                            })
                            .await
                        {
                            log::error!("Error sending delegate event: {}", err);
                        }
                    }
                });
                Ok(())
            },
        )
    }

    pub fn create_read_handler(
        &mut self,
        service_uuid: Uuid,
    ) -> TypedEventHandler<GattLocalCharacteristic, GattReadRequestedEventArgs> {
        let sender_tx: Sender<PeripheralEvent> = self.sender_tx.clone();

        TypedEventHandler::new(
            move |originator: &Option<GattLocalCharacteristic>,
                  args: &Option<GattReadRequestedEventArgs>| {
                let (Some(event_args), Some(characteristic)) = (args.as_ref(), originator.as_ref())
                else {
                    return Ok(());
                };
                let client = device_id_from_session(event_args.Session()?);
                let characteristic_uuid = to_uuid(&characteristic.Uuid()?);
                let request = event_args.GetRequestAsync()?;

                futures::executor::block_on(async {
                    let Ok(request) = request.await else {
                        return;
                    };
                    let offset = match request.Offset() {
                        Ok(offset) => offset as u64,
                        Err(err) => {
                            log::error!("Error reading request offset: {}", err);
                            respond_read_error(&request, RequestResponse::UnlikelyError);
                            return;
                        }
                    };

                    let (resp_tx, resp_rx) = oneshot::channel::<ReadRequestResponse>();
                    if let Err(e) = sender_tx
                        .send(PeripheralEvent::ReadRequest {
                            request: PeripheralRequest {
                                client,
                                service: service_uuid,
                                characteristic: characteristic_uuid,
                            },
                            offset,
                            responder: resp_tx,
                        })
                        .await
                    {
                        log::error!("Error sending delegate event: {}", e);
                        return;
                    }

                    let Ok(result) = resp_rx.await else {
                        respond_read_error(&request, RequestResponse::UnlikelyError);
                        return;
                    };
                    if result.response != RequestResponse::Success {
                        respond_read_error(&request, result.response);
                        return;
                    }

                    match vec_to_buffer(result.value) {
                        Ok(buffer) => {
                            if let Err(err) = request.RespondWithValue(&buffer) {
                                log::error!("Error responding to read request: {}", err);
                            }
                        }
                        Err(err) => {
                            log::error!("Error building read response buffer: {}", err);
                            respond_read_error(&request, RequestResponse::UnlikelyError);
                        }
                    }
                });

                return Ok(());
            },
        )
    }

    pub fn create_write_handler(
        &self,
        service_uuid: Uuid,
    ) -> TypedEventHandler<GattLocalCharacteristic, GattWriteRequestedEventArgs> {
        let sender_tx = self.sender_tx.clone();

        TypedEventHandler::new(
            move |originator: &Option<GattLocalCharacteristic>,
                  args: &Option<GattWriteRequestedEventArgs>| {
                let (Some(event_args), Some(characteristic)) = (args.as_ref(), originator.as_ref())
                else {
                    return Ok(());
                };
                let client = device_id_from_session(event_args.Session()?);
                let characteristic_uuid = to_uuid(&characteristic.Uuid()?);
                let request = event_args.GetRequestAsync()?;

                futures::executor::block_on(async {
                    let Ok(request) = request.await else {
                        return;
                    };
                    let value = match request.Value().and_then(|v| buffer_to_vec(&v)) {
                        Ok(value) => value,
                        Err(err) => {
                            log::error!("Error reading write request value: {}", err);
                            respond_write_error(&request, RequestResponse::UnlikelyError);
                            return;
                        }
                    };
                    let offset = match request.Offset() {
                        Ok(offset) => offset as u64,
                        Err(err) => {
                            log::error!("Error reading request offset: {}", err);
                            respond_write_error(&request, RequestResponse::UnlikelyError);
                            return;
                        }
                    };

                    let (resp_tx, resp_rx) = oneshot::channel::<WriteRequestResponse>();
                    if let Err(e) = sender_tx
                        .send(PeripheralEvent::WriteRequest {
                            request: PeripheralRequest {
                                client,
                                service: service_uuid,
                                characteristic: characteristic_uuid,
                            },
                            value,
                            offset,
                            responder: resp_tx,
                        })
                        .await
                    {
                        log::error!("Error sending delegate event: {}", e);
                        return;
                    }

                    let Ok(result) = resp_rx.await else {
                        respond_write_error(&request, RequestResponse::UnlikelyError);
                        return;
                    };
                    if result.response != RequestResponse::Success {
                        respond_write_error(&request, result.response);
                        return;
                    }

                    if let Err(err) = request.Respond() {
                        log::error!("Error responding to write request: {}", err);
                    }
                });

                return Ok(());
            },
        )
    }
}

fn respond_read_error(request: &GattReadRequest, response: RequestResponse) {
    if let Err(err) = request.RespondWithProtocolError(response.to_gatt_protocol_error()) {
        log::error!("Error responding to read request: {}", err);
    }
}

fn respond_write_error(request: &GattWriteRequest, response: RequestResponse) {
    if let Err(err) = request.RespondWithProtocolError(response.to_gatt_protocol_error()) {
        log::error!("Error responding to write request: {}", err);
    }
}

impl RequestResponse {
    fn to_gatt_protocol_error(self) -> u8 {
        let result = match self {
            RequestResponse::Success => Ok(0),
            RequestResponse::InvalidHandle => GattProtocolError::InvalidHandle(),
            RequestResponse::RequestNotSupported => GattProtocolError::RequestNotSupported(),
            RequestResponse::InvalidOffset => GattProtocolError::InvalidOffset(),
            RequestResponse::UnlikelyError => GattProtocolError::UnlikelyError(),
        };
        if let Ok(value) = result {
            return value;
        }
        return 0;
    }
}
