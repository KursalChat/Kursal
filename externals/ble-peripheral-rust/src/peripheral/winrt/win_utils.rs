use std::collections::HashMap;

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
    Uuid::from_fields(uuid.data1, uuid.data2, uuid.data3, &uuid.data4)
}

pub(crate) fn buffer_to_vec(buffer: &IBuffer) -> Result<Vec<u8>, Error> {
    let reader = DataReader::FromBuffer(buffer)?;
    let len = reader.UnconsumedBufferLength()? as usize;
    let mut data = vec![0u8; len];
    reader.ReadBytes(&mut data)?;
    Ok(data)
}

pub(crate) fn vec_to_buffer(vector: Vec<u8>) -> Result<IBuffer, Error> {
    let stream = InMemoryRandomAccessStream::new()?;
    let data_writer = DataWriter::CreateDataWriter(&stream)?;
    data_writer.WriteBytes(&vector)?;
    data_writer.DetachBuffer()
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
