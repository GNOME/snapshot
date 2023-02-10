#[derive(Debug)]
pub enum Error {
    NoCamera,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Could not find any camera")
    }
}
