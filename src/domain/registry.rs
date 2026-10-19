use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Um repositório do registry embutido, para a lista da sub-aba Registry.
/// `size_bytes` é a soma dos manifests do repo (aproximação — blobs podem ser
/// compartilhados entre repos via dedupe, ver `docs/plano-registry-embutido.md`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryRepoInfo {
    pub name: String,
    pub tag_count: i64,
    pub size_bytes: i64,
    pub created_at: DateTime<Utc>,
}

/// Uma tag de um repositório, com o manifest que ela aponta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryTagInfo {
    pub tag: String,
    pub digest: String,
    pub media_type: String,
    pub size_bytes: i64,
    pub updated_at: DateTime<Utc>,
}

/// Status do registry embutido: config + agregados, para o cabeçalho da
/// sub-aba Registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryStatusInfo {
    pub enabled: bool,
    pub port: u16,
    pub domain: Option<String>,
    pub repo_count: i64,
    pub blob_count: i64,
    pub storage_bytes: i64,
}

/// Um token de acesso do registry (Basic auth) — o segredo em texto plano só
/// aparece uma vez, em `RegistryTokenCreated`; aqui é só metadado.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryTokenInfo {
    pub name: String,
    pub scope: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

// ── Jobs (tarefas one-shot via docker-compose, agendadas ou manuais) ──────────
