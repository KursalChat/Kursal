use std::{collections::HashMap, str::FromStr};

use uuid::Uuid;
use windows::{
    core::{Error, GUID},
    Devices::Bluetooth::GenericAttributeProfile::{
        GattLocalCharacteristic, GattServiceProvider, GattSession, GattSubscribedClient,
    },
    Foundation::EventRegistrationToken,
    Storage::Streams::{DataReader, DataWriter, IBuffer, InMemoryRandomAccessStream},
};

pub struct GattCharacteristicObject {
    pub obj: GattLocalCharacteristic,
    pub subscribed_clients: Vec<GattSubscribedClient>,
    pub subscribed_clients_token: EventRegistrationToken,
    pub read_requested_token: EventRegistrationToken,
    pub write_requested_token: EventRegistrationToken,
}

pub struct GattServiceProviderObject {
    pub obj: GattServiceProvider,
    pub advertisement_status_changed_token: EventRegistrationToken,
    pub characteristics: HashMap<Uuid, GattCharacteristicObject>,
}

pub(crate) fn to_guid(uuid: &Uuid) -> GUID {
    let (g1, g2, g3, g4) = uuid.as_fields();
    GUID::from_values(g1, g2, g3, g4.clone())
}

pub(crate) fn to_uuid(uuid: &GUID) -> Uuid {
    let guid_s = format!("{:?}", uuid);
    Uuid::from_str(&guid_s).unwrap()
}

pub(crate) fn buffer_to_vec(buffer: &IBuffer) -> Vec<u8> {
    let reader = DataReader::FromBuffer(buffer).unwrap();
    let len = reader.UnconsumedBufferLength().unwrap() as usize;
    let mut data = vec![0u8; len];
    reader.ReadBytes(&mut data).unwrap();
    data
}

pub(crate) fn vec_to_buffer(vector: Vec<u8>) -> IBuffer {
    let stream = InMemoryRandomAccessStream::new().unwrap();
    let data_writer = DataWriter::CreateDataWriter(&stream).unwrap();
    data_writer.WriteBytes(&vector).unwrap();
    data_writer.DetachBuffer().unwrap()
}

pub(crate) fn device_id_from_session(session: GattSession) -> String {
    if let Ok(id) = get_complete_device_id(session) {
        if let Some(id) = id.split("-").last() {
            return id.to_string();
        }
        return id;
    }
    return "".to_string();
}

fn get_complete_device_id(session: GattSession) -> Result<String, Error> {
    if let Ok(id) = session.DeviceId()?.Id()?.to_os_string().into_string() {
        return Ok(id);
    }
    return Ok("".to_string());
}
