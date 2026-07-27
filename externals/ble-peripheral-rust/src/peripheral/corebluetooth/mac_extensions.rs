use objc2::rc::Retained;
use objc2_core_bluetooth::{CBCharacteristic, CBService, CBUUID};
use objc2_foundation::NSString;
use uuid::Uuid;

use crate::uuid::ShortUuid;

pub fn uuid_to_cbuuid(uuid: Uuid) -> Retained<CBUUID> {
    unsafe { CBUUID::UUIDWithString(&NSString::from_str(&uuid.to_string())) }
}

pub trait UuidExtension {
    fn get_uuid(self) -> Uuid;
}

impl UuidExtension for &CBService {
    fn get_uuid(self) -> Uuid {
        unsafe { self.UUID().get_uuid() }
    }
}

impl UuidExtension for &CBCharacteristic {
    fn get_uuid(self) -> Uuid {
        unsafe { self.UUID().get_uuid() }
    }
}

impl UuidExtension for &CBUUID {
    fn get_uuid(self) -> Uuid {
        let uuid_str = unsafe { self.UUIDString() }.to_string();
        return Uuid::from_string(uuid_str.as_str());
    }
}
