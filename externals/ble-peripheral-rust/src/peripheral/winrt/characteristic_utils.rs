use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristicProperties, GattProtectionLevel,
};

use crate::gatt::{
    characteristic::Characteristic,
    properties::{AttributePermission, CharacteristicProperty},
};

pub fn get_gatt_characteristic_properties(
    characteristic: &Characteristic,
) -> windows::core::Result<GattCharacteristicProperties> {
    return Ok(characteristic
        .properties
        .iter()
        .fold(GattCharacteristicProperties::None, |acc, property| {
            acc | property.clone().to_gatt_property()
        }));
}

pub fn get_protection_level(
    attribute_permission: Vec<AttributePermission>,
) -> windows::core::Result<(GattProtectionLevel, GattProtectionLevel)> {
    let mut write_protection_level = GattProtectionLevel::Plain;
    let mut read_protection_level = GattProtectionLevel::Plain;

    for permission in attribute_permission {
        if permission == AttributePermission::ReadEncryptionRequired {
            read_protection_level = GattProtectionLevel::EncryptionRequired;
        } else if permission == AttributePermission::WriteEncryptionRequired {
            write_protection_level = GattProtectionLevel::EncryptionRequired;
        }
    }

    Ok((write_protection_level, read_protection_level))
}

impl CharacteristicProperty {
    fn to_gatt_property(self) -> GattCharacteristicProperties {
        return match self {
            CharacteristicProperty::Broadcast => GattCharacteristicProperties::Broadcast,
            CharacteristicProperty::Read => GattCharacteristicProperties::Read,
            CharacteristicProperty::WriteWithoutResponse => {
                GattCharacteristicProperties::WriteWithoutResponse
            }
            CharacteristicProperty::Write => GattCharacteristicProperties::Write,
            CharacteristicProperty::Notify => GattCharacteristicProperties::Notify,

            CharacteristicProperty::Indicate => GattCharacteristicProperties::Indicate,

            CharacteristicProperty::AuthenticatedSignedWrites => {
                GattCharacteristicProperties::AuthenticatedSignedWrites
            }
            CharacteristicProperty::ExtendedProperties => {
                GattCharacteristicProperties::ExtendedProperties
            }
            CharacteristicProperty::IndicateEncryptionRequired => {
                GattCharacteristicProperties::Indicate // Alternative not available
            }
            CharacteristicProperty::NotifyEncryptionRequired => {
                GattCharacteristicProperties::Notify // Alternative not available
            }
        };
    }
}
