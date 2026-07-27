use super::properties::{AttributePermission, CharacteristicProperty};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Descriptor {
    pub uuid: Uuid,
    pub properties: Vec<CharacteristicProperty>,
    pub permissions: Vec<AttributePermission>,
    pub value: Option<Vec<u8>>,
}

impl Default for Descriptor {
    fn default() -> Self {
        Descriptor {
            uuid: Uuid::nil(),
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
        }
    }
}
