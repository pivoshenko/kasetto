pub(crate) type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn err(message: impl Into<String>) -> Error {
    std::io::Error::other(message.into()).into()
}
