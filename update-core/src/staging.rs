//! Cópia segura do pacote para um diretório que só o root controla.
//!
//! O daemon (sem privilégio) escreve o `.deb` em `/var/lib/rustploy/updates/`, e o
//! updater root **não** confere nem instala ali: copia primeiro para o seu próprio
//! diretório e só lê a cópia. Assim o daemon não consegue trocar o arquivo entre a
//! conferência da assinatura e a instalação (TOCTOU).

use std::{
    fmt, fs,
    io::{self, Read},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum StagingError {
    /// O arquivo não está direto dentro do diretório permitido (nem por link simbólico no caminho).
    OutsideAllowedDir,
    /// Nome de arquivo estranho (vazio, com `/`, começando por `.`…).
    BadFileName,
    /// Não é arquivo regular (é link simbólico, diretório, dispositivo…).
    NotRegularFile,
    TooLarge {
        max_bytes: u64,
    },
    Io(io::Error),
}

impl fmt::Display for StagingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideAllowedDir => {
                write!(formatter, "o arquivo não está no diretório de atualizações")
            }
            Self::BadFileName => write!(formatter, "nome de arquivo inválido"),
            Self::NotRegularFile => write!(
                formatter,
                "não é um arquivo regular (link simbólico ou outro tipo)"
            ),
            Self::TooLarge { max_bytes } => {
                write!(formatter, "arquivo maior que o limite de {max_bytes} bytes")
            }
            Self::Io(error) => write!(formatter, "erro de E/S: {error}"),
        }
    }
}

impl std::error::Error for StagingError {}

impl From<io::Error> for StagingError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._+-~".contains(character))
}

/// Copia `source` para `staging_dir` (criado `0700`, cópia `0600`) e devolve o
/// caminho da cópia.
///
/// - `source` tem de estar **direto** em `allowed_dir` (comparado depois de
///   resolver links do diretório), e o próprio arquivo é aberto com `O_NOFOLLOW`;
/// - tem de ser arquivo regular de no máximo `max_bytes` (o limite vale também
///   durante a cópia, caso o arquivo cresça depois do `stat`).
pub fn stage_file(
    source: &Path,
    allowed_dir: &Path,
    staging_dir: &Path,
    max_bytes: u64,
) -> Result<PathBuf, StagingError> {
    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| is_plain_file_name(name))
        .ok_or(StagingError::BadFileName)?;
    let parent = source.parent().ok_or(StagingError::OutsideAllowedDir)?;
    let canonical_parent = fs::canonicalize(parent).map_err(|_| StagingError::OutsideAllowedDir)?;
    let canonical_allowed =
        fs::canonicalize(allowed_dir).map_err(|_| StagingError::OutsideAllowedDir)?;
    if canonical_parent != canonical_allowed {
        return Err(StagingError::OutsideAllowedDir);
    }

    let source_file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(canonical_parent.join(file_name))
        .map_err(|error| match error.raw_os_error() {
            // ELOOP: o último componente é um link simbólico.
            Some(code) if code == libc::ELOOP => StagingError::NotRegularFile,
            _ => StagingError::Io(error),
        })?;
    let metadata = source_file.metadata()?;
    if !metadata.is_file() {
        return Err(StagingError::NotRegularFile);
    }
    if metadata.len() > max_bytes {
        return Err(StagingError::TooLarge { max_bytes });
    }

    fs::create_dir_all(staging_dir)?;
    fs::set_permissions(staging_dir, fs::Permissions::from_mode(0o700))?;
    let destination = staging_dir.join(file_name);
    // Sobra de uma tentativa anterior: sai antes, para o `create_new` valer de verdade.
    let _ = fs::remove_file(&destination);
    let mut destination_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&destination)?;

    // `take(max + 1)`: se passar de `max`, o arquivo cresceu depois do `stat`.
    let copied = io::copy(
        &mut (&source_file).take(max_bytes + 1),
        &mut destination_file,
    )?;
    if copied > max_bytes {
        let _ = fs::remove_file(&destination);
        return Err(StagingError::TooLarge { max_bytes });
    }
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct Dirs {
        _root: tempfile::TempDir,
        allowed: PathBuf,
        staging: PathBuf,
    }

    fn dirs() -> Dirs {
        let root = tempfile::tempdir().unwrap();
        let allowed = root.path().join("updates");
        let staging = root.path().join("staging");
        fs::create_dir_all(&allowed).unwrap();
        Dirs {
            _root: root,
            allowed,
            staging,
        }
    }

    #[test]
    fn copies_a_regular_file_with_private_permissions() {
        let dirs = dirs();
        let source = dirs.allowed.join("rustployd_0.1.10-1_amd64.deb");
        fs::write(&source, b"conteudo").unwrap();

        let staged = stage_file(&source, &dirs.allowed, &dirs.staging, 1024).unwrap();

        assert_eq!(fs::read(&staged).unwrap(), b"conteudo");
        assert_eq!(
            fs::metadata(&staged).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&dirs.staging).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    #[test]
    fn the_copy_does_not_follow_later_changes_to_the_source() {
        let dirs = dirs();
        let source = dirs.allowed.join("a.deb");
        fs::write(&source, b"original").unwrap();
        let staged = stage_file(&source, &dirs.allowed, &dirs.staging, 1024).unwrap();
        fs::write(&source, b"trocado depois").unwrap();
        assert_eq!(fs::read(&staged).unwrap(), b"original");
    }

    #[test]
    fn a_symlink_as_the_file_is_refused() {
        let dirs = dirs();
        let secret = dirs.allowed.parent().unwrap().join("segredo");
        fs::write(&secret, b"nao copiar").unwrap();
        let link = dirs.allowed.join("a.deb");
        symlink(&secret, &link).unwrap();
        assert!(matches!(
            stage_file(&link, &dirs.allowed, &dirs.staging, 1024),
            Err(StagingError::NotRegularFile)
        ));
    }

    #[test]
    fn a_file_outside_the_allowed_dir_is_refused() {
        let dirs = dirs();
        let outside = dirs.allowed.parent().unwrap().join("fora.deb");
        fs::write(&outside, b"x").unwrap();
        assert!(matches!(
            stage_file(&outside, &dirs.allowed, &dirs.staging, 1024),
            Err(StagingError::OutsideAllowedDir)
        ));
    }

    #[test]
    fn dot_dot_and_symlinked_directories_do_not_escape() {
        let dirs = dirs();
        let outside_dir = dirs.allowed.parent().unwrap().join("outro");
        fs::create_dir_all(&outside_dir).unwrap();
        fs::write(outside_dir.join("a.deb"), b"x").unwrap();

        let dotdot = dirs.allowed.join("..").join("outro").join("a.deb");
        assert!(matches!(
            stage_file(&dotdot, &dirs.allowed, &dirs.staging, 1024),
            Err(StagingError::OutsideAllowedDir)
        ));

        let linked_dir = dirs.allowed.parent().unwrap().join("atalho");
        symlink(&outside_dir, &linked_dir).unwrap();
        assert!(matches!(
            stage_file(
                &linked_dir.join("a.deb"),
                &dirs.allowed,
                &dirs.staging,
                1024
            ),
            Err(StagingError::OutsideAllowedDir)
        ));
    }

    #[test]
    fn a_directory_or_a_missing_file_is_not_staged() {
        let dirs = dirs();
        fs::create_dir_all(dirs.allowed.join("subdir.deb")).unwrap();
        assert!(matches!(
            stage_file(
                &dirs.allowed.join("subdir.deb"),
                &dirs.allowed,
                &dirs.staging,
                1024
            ),
            Err(StagingError::NotRegularFile)
        ));
        assert!(matches!(
            stage_file(
                &dirs.allowed.join("nao-existe.deb"),
                &dirs.allowed,
                &dirs.staging,
                1024
            ),
            Err(StagingError::Io(_))
        ));
    }

    #[test]
    fn an_oversized_file_is_refused_and_leaves_nothing_behind() {
        let dirs = dirs();
        let source = dirs.allowed.join("grande.deb");
        fs::write(&source, vec![0u8; 100]).unwrap();
        assert!(matches!(
            stage_file(&source, &dirs.allowed, &dirs.staging, 99),
            Err(StagingError::TooLarge { max_bytes: 99 })
        ));
        assert!(!dirs.staging.join("grande.deb").exists());
        // No limite exato passa.
        assert!(stage_file(&source, &dirs.allowed, &dirs.staging, 100).is_ok());
    }

    #[test]
    fn odd_file_names_are_refused() {
        let dirs = dirs();
        for name in [".oculto.deb", "com espaco.deb", "a;b.deb"] {
            let source = dirs.allowed.join(name);
            fs::write(&source, b"x").unwrap();
            assert!(
                matches!(
                    stage_file(&source, &dirs.allowed, &dirs.staging, 1024),
                    Err(StagingError::BadFileName)
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn restaging_replaces_a_leftover_copy() {
        let dirs = dirs();
        let source = dirs.allowed.join("a.deb");
        fs::write(&source, b"um").unwrap();
        stage_file(&source, &dirs.allowed, &dirs.staging, 1024).unwrap();
        fs::write(&source, b"dois").unwrap();
        let staged = stage_file(&source, &dirs.allowed, &dirs.staging, 1024).unwrap();
        assert_eq!(fs::read(&staged).unwrap(), b"dois");
    }
}
