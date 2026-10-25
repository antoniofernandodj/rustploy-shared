//! Comparação de versões de release (semver), tolerante ao `v` das tags.

use semver::Version;
use std::fmt;

/// A entrada não é uma versão semver (depois de tirar espaços e o `v`/`V` inicial).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionError {
    input: String,
}

impl fmt::Display for VersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "versão inválida: {:?}", self.input)
    }
}

impl std::error::Error for VersionError {}

/// Lê `0.1.15`, `v0.1.15` ou `V0.1.15` (com ou sem espaços ao redor).
pub fn parse_version(text: &str) -> Result<Version, VersionError> {
    let trimmed = text.trim();
    let without_prefix = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);
    Version::parse(without_prefix).map_err(|_| VersionError {
        input: text.to_string(),
    })
}

/// `true` se `candidate` é **estritamente** mais nova que `installed`.
/// Comparação numérica por semver (`0.1.9` < `0.1.15`); um pré-lançamento
/// (`0.2.0-rc1`) é menor que a versão final (`0.2.0`).
pub fn is_newer(candidate: &str, installed: &str) -> Result<bool, VersionError> {
    Ok(parse_version(candidate)? > parse_version(installed)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_numerically_not_as_text() {
        assert!(is_newer("0.1.15", "0.1.9").unwrap());
        assert!(!is_newer("0.1.9", "0.1.15").unwrap());
        assert!(is_newer("0.10.0", "0.9.9").unwrap());
        assert!(is_newer("1.0.0", "0.99.99").unwrap());
    }

    #[test]
    fn equal_versions_are_not_newer() {
        assert!(!is_newer("0.1.7", "0.1.7").unwrap());
        assert!(!is_newer("v0.1.7", "0.1.7").unwrap());
    }

    #[test]
    fn accepts_tag_with_and_without_v() {
        assert!(is_newer("v0.2.0", "0.1.9").unwrap());
        assert!(is_newer("0.2.0", "v0.1.9").unwrap());
        assert!(is_newer("V0.2.0", " 0.1.9 ").unwrap());
    }

    #[test]
    fn prerelease_is_older_than_final() {
        assert!(!is_newer("0.2.0-rc1", "0.2.0").unwrap());
        assert!(is_newer("0.2.0", "0.2.0-rc1").unwrap());
        assert!(is_newer("0.2.0-rc2", "0.2.0-rc1").unwrap());
    }

    #[test]
    fn build_metadata_still_parses() {
        assert!(parse_version("v0.1.7+abc").is_ok());
    }

    #[test]
    fn rejects_garbage() {
        for input in ["", "v", "latest", "0.1", "v1.x.3", "0.1.7.1"] {
            assert!(parse_version(input).is_err(), "{input:?} deveria falhar");
        }
        assert!(is_newer("latest", "0.1.0").is_err());
        assert!(is_newer("0.1.0", "").is_err());
    }

    #[test]
    fn error_message_shows_the_input() {
        let error = parse_version("abc").unwrap_err();
        assert!(error.to_string().contains("abc"));
    }
}
