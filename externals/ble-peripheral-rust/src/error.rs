use std::{error, fmt};

#[derive(Debug, Clone)]
pub enum ErrorType {
    Bluez,
    CoreBluetooth,
    Windows,
    PermissionDenied,
    ChannelError,
}

impl From<ErrorType> for &'static str {
    fn from(error_type: ErrorType) -> &'static str {
        match error_type {
            ErrorType::Bluez => "Bluez",
            ErrorType::CoreBluetooth => "CoreBluetooth",
            ErrorType::Windows => "Windows",
            ErrorType::PermissionDenied => "PermissionDenied",
            ErrorType::ChannelError => "ChannelError",
        }
    }
}

impl fmt::Display for ErrorType {
    fn fmt(self: &Self, f: &mut fmt::Formatter) -> fmt::Result {
        let error_type: &str = self.clone().into();
        write!(f, "<BlePeripheralRust {} Error>", error_type)
    }
}

impl error::Error for ErrorType {}

#[derive(Debug, Clone)]
pub struct Error {
    name: String,
    description: String,
    combined_description: String,
    error_type: ErrorType,
}

impl Error {
    pub fn new<T: Into<String>>(name: T, description: T, error_type: ErrorType) -> Self {
        let name: String = name.into();
        let description: String = description.into();
        let combined_description = format!("{}: {}", name, description);
        Error {
            name,
            description,
            combined_description,
            error_type,
        }
    }

    pub fn from_type(error_type: ErrorType) -> Self {
        let name: String = error_type.to_string();
        let description: String = error_type.to_string();
        let combined_description = format!("{}: {}", name, description);
        Error {
            name,
            description,
            combined_description,
            error_type,
        }
    }

    pub fn from_string(error: String, error_type: ErrorType) -> Self {
        let name: String = error_type.to_string();
        let description: String = error;
        let combined_description = format!("{}: {}", name, description);
        Error {
            name,
            description,
            combined_description,
            error_type,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(self: &Self, f: &mut fmt::Formatter) -> fmt::Result {
        let error_type: &str = self.error_type.clone().into();
        write!(
            f,
            "**BlePeripheralRust {} Error**\n\n\t{}:\n\t\t{}",
            error_type, self.name, self.description,
        )
    }
}

impl error::Error for Error {
    fn description(self: &Self) -> &str {
        &self.combined_description
    }

    fn source(self: &Self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.error_type)
    }
}
