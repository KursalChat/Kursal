use uuid::Uuid;

pub trait ShortUuid {
    fn from_short(uuid: u16) -> Uuid;

    fn from_string(uuid_str: &str) -> Uuid;
}

impl ShortUuid for Uuid {
    fn from_short(uuid: u16) -> Uuid {
        return Uuid::from_fields(uuid.into(), 0, 0x1000, b"\x80\x00\x00\x80\x5F\x9B\x34\xFB");
    }

    fn from_string(uuid_str: &str) -> Uuid {
        let uuid = uuid_str.to_string();
        match Uuid::parse_str(&uuid.clone()) {
            Ok(uuid) => uuid,
            Err(_) => {
                let long_uuid_str = match uuid.len() {
                    4 => format!("0000{}-0000-1000-8000-00805f9b34fb", uuid),
                    8 => format!("{}-0000-1000-8000-00805f9b34fb", uuid),
                    _ => uuid.clone(),
                };
                Uuid::parse_str(&long_uuid_str)
                    .unwrap_or_else(|_| panic!("Invalid UUID string: {}", uuid))
            }
        }
    }
}
