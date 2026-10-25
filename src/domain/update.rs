//! Verificação de versão: o que o daemon responde a `Command::CheckUpdate`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Resultado de comparar a versão do daemon com a última release publicada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// Versão do daemon em execução (`CARGO_PKG_VERSION`).
    pub current_version: String,
    /// Versão da última release estável, sem o `v` da tag.
    pub latest_version: String,
    /// `true` só se `latest_version` for estritamente mais nova (semver).
    pub is_update_available: bool,
    /// Página da release (notas e arquivos).
    pub release_url: String,
    /// Quando a release foi publicada, se o GitHub informou.
    pub published_at: Option<DateTime<Utc>>,
    /// Quando o daemon consultou de fato (não quando respondeu: a resposta pode vir do cache).
    pub checked_at: DateTime<Utc>,
}
