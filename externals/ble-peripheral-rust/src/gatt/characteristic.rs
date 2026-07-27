use super::{
    descriptor::Descriptor,
    properties::{AttributePermission, CharacteristicProperty},
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Characteristic {
    pub uuid: Uuid,
    pub properties: Vec<CharacteristicProperty>,
    pub permissions: Vec<AttributePermission>,
    pub value: Option<Vec<u8>>,
    pub descriptors: Vec<Descriptor>,
}

impl Default for Characteristic {
    fn default() -> Self {
        Characteristic {
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
            descriptors: Vec::new(),
        }
    }
}
