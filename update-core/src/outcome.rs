//! O resultado da última atualização, gravado pelo updater (root) e lido pelo
//! daemon depois de reiniciar: é como a UI descobre o *porquê* de uma falha,
//! já que a conexão com o daemon cai no meio da instalação.

use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs, io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeKind {
    /// O pacote foi aceito (assinatura e regras ok) e está sendo instalado agora.
    InProgress,
    /// Instalou e o serviço ficou estável.
    Installed,
    /// Recusou ou falhou antes de mudar qualquer coisa.
    Failed,
    /// Instalou, o serviço não ficou estável e os binários anteriores foram restaurados.
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateOutcome {
    pub package: String,
    /// Versão do pacote pedido (a que o `.deb` diz ter).
    pub version: String,
    pub kind: OutcomeKind,
    /// Texto para a pessoa: a causa de uma falha, ou "instalado".
    pub message: String,
    /// Segundos desde a época Unix.
    pub at_unix: u64,
}

impl UpdateOutcome {
    pub fn new(
        package: &str,
        version: &str,
        kind: OutcomeKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            package: package.to_string(),
            version: version.to_string(),
            kind,
            message: message.into(),
            at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_secs())
                .unwrap_or(0),
        }
    }
}

/// Grava de forma atômica (arquivo temporário + `rename`) e legível por todos:
/// o daemon (usuário `rustploy`) precisa lê-lo, e ele não tem segredo.
pub fn write_outcome(path: &Path, outcome: &UpdateOutcome) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(outcome).map_err(io::Error::other)?,
    )?;
    // Legível por todos (o daemon, que não é root, precisa lê-lo). Permissão POSIX: no Windows, onde
    // este módulo também compila (o app é compilado para lá), não há o que ajustar.
    #[cfg(unix)]
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o644))?;
    fs::rename(&temporary, path)
}

/// `None` se ainda não houve nenhuma atualização; erro se o arquivo existir e estiver ilegível.
pub fn read_outcome(path: &Path) -> io::Result<Option<UpdateOutcome>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("estado").join("last.json");
        let outcome = UpdateOutcome::new(
            "rustployd",
            "0.1.10-1",
            OutcomeKind::RolledBack,
            "serviço caiu",
        );
        write_outcome(&path, &outcome).unwrap();
        assert_eq!(read_outcome(&path).unwrap(), Some(outcome));
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn a_missing_file_is_none_and_a_corrupt_one_is_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("last.json");
        assert_eq!(read_outcome(&path).unwrap(), None);
        fs::write(&path, b"{ nao e json").unwrap();
        assert!(read_outcome(&path).is_err());
    }

    #[test]
    fn serializes_kind_in_snake_case_for_other_readers() {
        let outcome = UpdateOutcome::new("p", "1", OutcomeKind::RolledBack, "x");
        assert!(
            serde_json::to_string(&outcome)
                .unwrap()
                .contains("\"rolled_back\"")
        );
    }
}
