#[derive(Debug)]
pub enum Error {
    NoCamera,
    DeviceProvider,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoCamera => f.write_str("Could not find any camera"),
            Error::DeviceProvider => f.write_str("Could not initialize the device provider"),
        }
    }
}
