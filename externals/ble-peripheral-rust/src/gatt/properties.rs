#[derive(Debug, Clone, PartialEq)]
pub enum CharacteristicProperty {
    Broadcast,
    Read,
    WriteWithoutResponse,
    Write,
    AuthenticatedSignedWrites,
    Notify,
    NotifyEncryptionRequired,
    Indicate,
    IndicateEncryptionRequired,
    ExtendedProperties,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttributePermission {
    Readable,
    Writeable,
    ReadEncryptionRequired,
    WriteEncryptionRequired,
}
