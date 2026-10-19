//! Protocolo da API: `Command` (o que o cliente pede), `Response` e `Event` (o que o SSE entrega).

mod command;
mod event;
mod logs;
mod response;

pub use command::Command;
pub use event::Event;
pub use logs::{BuildLogLine, LogEntry, LogStream};
pub use response::Response;
