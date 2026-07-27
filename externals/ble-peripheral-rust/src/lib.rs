pub mod error;
pub mod gatt;
pub mod uuid;

mod peripheral;
pub use self::peripheral::{Peripheral, PeripheralImpl};
