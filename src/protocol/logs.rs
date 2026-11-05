//! Linhas de log: stream de origem e entradas de container/build.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub stream: LogStream,
    pub line: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Uma linha do log de runtime **persistido** (`LogsHistory`): além do texto, de qual container veio,
/// porque o histórico atravessa trocas de container (deploys).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogHistoryEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Id curto (12 caracteres) do container que gerou a linha.
    pub container_id: String,
    pub stream: LogStream,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildLogLine {
    pub stream: LogStream,
    pub line: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
