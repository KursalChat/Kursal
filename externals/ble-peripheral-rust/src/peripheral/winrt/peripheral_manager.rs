use super::characteristic_utils::{get_gatt_characteristic_properties, get_protection_level};
use super::win_event_handler::WinEventHandler;
use super::win_utils::{
    to_guid, vec_to_buffer, GattCharacteristicObject, GattServiceProviderObject,
};
use crate::gatt::peripheral_event::PeripheralEvent;
use crate::gatt::service::Service;
use std::collections::HashMap;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;
use windows::core::{Error, HRESULT};
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattLocalCharacteristicParameters, GattLocalDescriptorParameters, GattServiceProvider,
    GattServiceProviderAdvertisementStatus, GattServiceProviderAdvertisingParameters,
    GattSubscribedClient,
};
use windows::Devices::Bluetooth::{BluetoothAdapter, BluetoothError};
use windows::Devices::Radios::{Radio, RadioKind};
use windows::Foundation::EventRegistrationToken;

pub(crate) struct PeripheralManager {
    event_handler: WinEventHandler,
    services: HashMap<Uuid, GattServiceProviderObject>,
}

impl PeripheralManager {
    pub(crate) async fn new(sender_tx: Sender<PeripheralEvent>) -> Self {
        let manager = Self {
            event_handler: WinEventHandler::new(sender_tx.clone()),
            services: HashMap::new(),
        };
        if let Err(err) = manager.set_radio_listener().await {
            log::error!("Error setting radio listener: {}", err);
        }
        return manager;
    }

    async fn set_radio_listener(&self) -> windows::core::Result<()> {
        let radios = Radio::GetRadiosAsync()?.await?;
        for radio in radios {
            if radio.Kind()? == RadioKind::Bluetooth {
                radio.StateChanged(&self.event_handler.create_radio_listener())?;
            }
        }
        return Ok(());
    }

    pub(crate) async fn is_powered(&self) -> windows::core::Result<bool> {
        let adapter: BluetoothAdapter = BluetoothAdapter::GetDefaultAsync()?.await?;
        let radio = adapter.GetRadioAsync()?.await?;
        radio.State().map(|state| state.0 == 1)
    }

    pub(crate) async fn is_advertising(&self) -> windows::core::Result<bool> {
        if self.services.is_empty() {
            return Ok(false);
        }
        return Ok(self.are_all_services_started()?);
    }

    pub(crate) async fn start_advertising(&self, _: &str, _: &[Uuid]) -> windows::core::Result<()> {
        let advertisement_parameter = GattServiceProviderAdvertisingParameters::new()?;
        advertisement_parameter.SetIsDiscoverable(true)?;
        advertisement_parameter.SetIsConnectable(true)?;

        // TODO: add name and uuid in advertisement or change adapter name
        for gatt_object in self.services.values().into_iter() {
            if gatt_object.obj.AdvertisementStatus()?
                == GattServiceProviderAdvertisementStatus::Started
            {
                log::debug!("Already advertising");
                continue;
            }
            gatt_object
                .obj
                .StartAdvertisingWithParameters(&advertisement_parameter)?;
        }

        Ok(())
    }

    pub(crate) async fn stop_advertising(&self) -> windows::core::Result<()> {
        for gatt_object in self.services.values().into_iter() {
            if gatt_object.obj.AdvertisementStatus()?
                != GattServiceProviderAdvertisementStatus::Stopped
            {
                gatt_object.obj.StopAdvertising()?;
            }
        }
        Ok(())
    }

    pub(crate) async fn add_service(&mut self, service: &Service) -> windows::core::Result<()> {
        // Create GattServiceProvider
        let service_uuid = to_guid(&service.uuid);
        let service_provider_result =
            GattServiceProvider::CreateAsync(service_uuid.into())?.await?;
        if service_provider_result.Error()? != BluetoothError::Success {
            return Err(Error::new(HRESULT(1), "Error getting GattServiceProvider"));
        }

        let service_provider = service_provider_result.ServiceProvider()?;

        let mut chars_map: HashMap<Uuid, GattCharacteristicObject> = HashMap::new();

        for characteristic in &service.characteristics {
            // Create LocalChar
            let uuid = to_guid(&characteristic.uuid);
            let parameters: GattLocalCharacteristicParameters =
                GattLocalCharacteristicParameters::new()?;

            let properties = get_gatt_characteristic_properties(characteristic)?;
            let (write_protection_level, read_protection_level) =
                get_protection_level(characteristic.permissions.clone())?;
            parameters.SetCharacteristicProperties(properties)?;
            parameters.SetWriteProtectionLevel(write_protection_level)?;
            parameters.SetReadProtectionLevel(read_protection_level)?;
            if let Some(value) = &characteristic.value {
                parameters.SetStaticValue(&vec_to_buffer(value.clone()))?;
            }

            // Add characteristic to Service provider
            let characteristic_result = service_provider
                .Service()?
                .CreateCharacteristicAsync(uuid.into(), &parameters)?
                .await?;

            if characteristic_result.Error()? != BluetoothError::Success {
                return Err(Error::new(HRESULT(1), "Error creating a characteristic"));
            }

            let win_characteristic = characteristic_result.Characteristic()?;

            // Add descriptor
            for descriptor in &characteristic.descriptors {
                let descriptoruuid = to_guid(&descriptor.uuid);
                let parameters: GattLocalDescriptorParameters =
                    GattLocalDescriptorParameters::new()?;
                let (write_protection_level, read_protection_level) =
                    get_protection_level(descriptor.permissions.clone())?;
                parameters.SetWriteProtectionLevel(write_protection_level)?;
                parameters.SetReadProtectionLevel(read_protection_level)?;

                if let Some(value) = &descriptor.value {
                    parameters.SetStaticValue(&vec_to_buffer(value.clone()))?;
                }

                let descriptor_result = win_characteristic
                    .CreateDescriptorAsync(descriptoruuid, &parameters)?
                    .await?;

                if descriptor_result.Error()? != BluetoothError::Success {
                    return Err(Error::new(HRESULT(1), "Error creating a descriptor"));
                }

                descriptor_result.Descriptor()?;
            }

            let read_token: Result<EventRegistrationToken, Error> = win_characteristic
                .ReadRequested(&self.event_handler.create_read_handler(service.uuid));
            let write_token: Result<EventRegistrationToken, Error> = win_characteristic
                .WriteRequested(&self.event_handler.create_write_handler(service.uuid));
            let subscribed_clients_token = win_characteristic.SubscribedClientsChanged(
                &self.event_handler.create_subscribe_handler(service.uuid),
            );

            let current_subscribed_clients: Vec<GattSubscribedClient> = win_characteristic
                .SubscribedClients()?
                .into_iter()
                .map(|x| x)
                .collect();

            let gatt_characteristic_object = GattCharacteristicObject {
                obj: win_characteristic.clone(),
                subscribed_clients: current_subscribed_clients,
                subscribed_clients_token: subscribed_clients_token?,
                read_requested_token: read_token?,
                write_requested_token: write_token?,
            };

            chars_map.insert(characteristic.uuid, gatt_characteristic_object);
        }

        let service_advertisement_changed = service_provider
            .AdvertisementStatusChanged(&self.event_handler.create_advertisement_status_handler());

        {
            let gatt_service_provider_object = GattServiceProviderObject {
                obj: service_provider.clone(),
                advertisement_status_changed_token: service_advertisement_changed?,
                characteristics: chars_map,
            };
            self.services
                .insert(service.uuid, gatt_service_provider_object);
        }
        Ok(())
    }

    pub async fn update_characteristic(
        &mut self,
        characteristic: Uuid,
        value: Vec<u8>,
    ) -> Result<(), Error> {
        let char = self
            .services
            .values()
            .find_map(|service| service.characteristics.get(&characteristic).map(|c| &c.obj))
            .expect("Characteristic not found");
        let notify_async = char.NotifyValueAsync(&vec_to_buffer(value))?;
        notify_async.await?;
        return Ok(());
    }

    fn are_all_services_started(&self) -> windows::core::Result<bool> {
        for service in self.services.values() {
            if service.obj.AdvertisementStatus()? != GattServiceProviderAdvertisementStatus::Started
            {
                return Ok(false);
            }
        }
        return Ok(true);
    }
}
