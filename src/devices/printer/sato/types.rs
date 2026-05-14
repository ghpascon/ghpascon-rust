#[derive(Debug, Clone)]
pub enum SatoEvent {
    Connection(bool),
    Status(String),
    Error(String),
    PrintSent(String),
}
