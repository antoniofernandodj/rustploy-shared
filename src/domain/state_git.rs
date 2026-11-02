//! Sincronização do estado de infra com um repositório git (plano
//! `docs/plano-backup-estado-em-git.md`, R6).

use serde::{Deserialize, Serialize};

/// Um commit do repositório de estado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateGitCommit {
    pub sha: String,
    /// RFC 3339.
    pub at: String,
    pub subject: String,
}

/// Situação da sincronização, para a tela de Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateGitStatus {
    /// `off` | `commit` | `sync`.
    pub mode: String,
    pub remote: Option<String>,
    pub branch: String,
    pub provider_id: Option<String>,
    /// O binário `git` existe no servidor?
    pub git_available: bool,
    /// `[secrets] recovery_recipient` está configurada? (sem ela, perder a chave perde os segredos)
    pub recovery_configured: bool,
    pub last_commit: Option<StateGitCommit>,
    /// Resultado do último `push` (`None` = nunca tentou).
    pub last_push_ok: Option<bool>,
    pub last_push_at: Option<String>,
    /// Último erro (git, rede, credencial). Não derruba nada: o daemon tenta de novo.
    pub last_error: Option<String>,
    /// Quantos valores em claro estão **impedindo** o commit agora (0 = livre).
    pub blocked_by_plaintext: usize,
    /// Há commits locais ainda não enviados.
    pub needs_push: bool,
}
