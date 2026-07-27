use super::characteristic::Characteristic;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Service {
    pub uuid: Uuid,
    pub primary: bool,
    pub characteristics: Vec<Characteristic>,
}

impl Default for Service {
    fn default() -> Self {
        Service {
            uuid: Uuid::nil(),
            primary: true,
            characteristics: Vec::new(),
        }
    }
}
