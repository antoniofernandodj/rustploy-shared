//! A decisão "este pacote assinado pode ser instalado aqui?", como função pura:
//! cruza o que a **assinatura** afirma, o que o **`.deb`** diz de si e o que a
//! **máquina** espera. Qualquer divergência recusa.

use crate::{dpkg_version, signature::SignedClaims};
use std::fmt;

/// Campos lidos do `.deb` com `dpkg-deb -f <arquivo> Package Version Architecture`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFacts {
    pub package: String,
    pub version: String,
    pub architecture: String,
}

/// O que este updater aceita instalar.
#[derive(Debug, Clone, Copy)]
pub struct Expectations<'a> {
    /// Nome fixo do pacote deste updater (`rustployd` ou `rustploy-gui`).
    pub package: &'a str,
    /// Saída de `dpkg --print-architecture` desta máquina.
    pub machine_architecture: &'a str,
    /// Versão hoje instalada, se houver (`dpkg-query -W -f '${Version}' <pacote>`).
    pub installed_version: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageError {
    /// O `.deb` é de outro pacote (mesmo assinado, não é o que este updater instala).
    WrongPackage { found: String, expected: String },
    /// A assinatura afirma um pacote diferente do que o `.deb` diz ser.
    ClaimsMismatch {
        field: &'static str,
        signed: String,
        actual: String,
    },
    /// O `.deb` é de outra arquitetura.
    WrongArchitecture { found: String, machine: String },
    /// Não é mais novo que o instalado (igual ou downgrade).
    NotNewer {
        candidate: String,
        installed: String,
    },
    /// Alguma das versões não é versão de pacote.
    BadVersion(String),
    /// A saída do `dpkg-deb -f` não trouxe os três campos.
    UnreadableFields,
}

impl fmt::Display for PackageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongPackage { found, expected } => {
                write!(
                    formatter,
                    "pacote errado: o arquivo é {found:?}, este updater só instala {expected:?}"
                )
            }
            Self::ClaimsMismatch {
                field,
                signed,
                actual,
            } => write!(
                formatter,
                "a assinatura diz {field}={signed:?}, mas o pacote diz {actual:?}"
            ),
            Self::WrongArchitecture { found, machine } => write!(
                formatter,
                "arquitetura errada: o pacote é {found:?} e esta máquina é {machine:?}"
            ),
            Self::NotNewer {
                candidate,
                installed,
            } => write!(
                formatter,
                "a versão {candidate} não é mais nova que a instalada ({installed}); downgrade é recusado"
            ),
            Self::BadVersion(detail) => write!(formatter, "{detail}"),
            Self::UnreadableFields => write!(
                formatter,
                "não consegui ler Package, Version e Architecture do pacote"
            ),
        }
    }
}

impl std::error::Error for PackageError {}

/// Lê a saída de `dpkg-deb -f <arquivo> Package Version Architecture`
/// (linhas `Campo: valor`, em qualquer ordem).
pub fn parse_dpkg_fields(output: &str) -> Result<PackageFacts, PackageError> {
    let (mut package, mut version, mut architecture) = (None, None, None);
    for line in output.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().to_string();
        match name.trim() {
            "Package" => package = Some(value),
            "Version" => version = Some(value),
            "Architecture" => architecture = Some(value),
            _ => {}
        }
    }
    match (package, version, architecture) {
        (Some(package), Some(version), Some(architecture)) => Ok(PackageFacts {
            package,
            version,
            architecture,
        }),
        _ => Err(PackageError::UnreadableFields),
    }
}

/// Recusa se qualquer regra falhar. A ordem põe as perguntas baratas e mais
/// reveladoras primeiro (pacote errado antes de versão).
pub fn check_package(
    claims: &SignedClaims,
    facts: &PackageFacts,
    expectations: &Expectations<'_>,
) -> Result<(), PackageError> {
    if facts.package != expectations.package {
        return Err(PackageError::WrongPackage {
            found: facts.package.clone(),
            expected: expectations.package.to_string(),
        });
    }
    for (field, signed, actual) in [
        ("package", &claims.package, &facts.package),
        ("version", &claims.version, &facts.version),
        ("arch", &claims.arch, &facts.architecture),
    ] {
        if signed != actual {
            return Err(PackageError::ClaimsMismatch {
                field,
                signed: signed.clone(),
                actual: actual.clone(),
            });
        }
    }
    if facts.architecture != expectations.machine_architecture && facts.architecture != "all" {
        return Err(PackageError::WrongArchitecture {
            found: facts.architecture.clone(),
            machine: expectations.machine_architecture.to_string(),
        });
    }
    if let Some(installed) = expectations.installed_version {
        let newer = dpkg_version::is_newer(&facts.version, installed)
            .map_err(|error| PackageError::BadVersion(error.to_string()))?;
        if !newer {
            return Err(PackageError::NotNewer {
                candidate: facts.version.clone(),
                installed: installed.to_string(),
            });
        }
    } else {
        // Sem versão instalada ainda precisa ser uma versão válida.
        dpkg_version::compare(&facts.version, &facts.version)
            .map_err(|error| PackageError::BadVersion(error.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(package: &str, version: &str, arch: &str) -> SignedClaims {
        SignedClaims {
            package: package.into(),
            version: version.into(),
            arch: arch.into(),
        }
    }

    fn facts(package: &str, version: &str, architecture: &str) -> PackageFacts {
        PackageFacts {
            package: package.into(),
            version: version.into(),
            architecture: architecture.into(),
        }
    }

    fn expect<'a>(installed: Option<&'a str>) -> Expectations<'a> {
        Expectations {
            package: "rustployd",
            machine_architecture: "amd64",
            installed_version: installed,
        }
    }

    #[test]
    fn a_newer_matching_package_is_accepted() {
        let result = check_package(
            &claims("rustployd", "0.1.10-1", "amd64"),
            &facts("rustployd", "0.1.10-1", "amd64"),
            &expect(Some("0.1.9-1")),
        );
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn a_signed_package_of_another_product_is_refused() {
        // O rustploy-gui é legítimo e assinado, mas não é o que o updater do daemon instala.
        let result = check_package(
            &claims("rustploy-gui", "0.1.20-1", "amd64"),
            &facts("rustploy-gui", "0.1.20-1", "amd64"),
            &expect(Some("0.1.9-1")),
        );
        assert!(matches!(result, Err(PackageError::WrongPackage { .. })));
    }

    #[test]
    fn every_signed_claim_must_match_the_package() {
        for (signed, field) in [
            (claims("outro", "0.1.10-1", "amd64"), "package"),
            (claims("rustployd", "0.9.9-1", "amd64"), "version"),
            (claims("rustployd", "0.1.10-1", "arm64"), "arch"),
        ] {
            let result = check_package(
                &signed,
                &facts("rustployd", "0.1.10-1", "amd64"),
                &expect(Some("0.1.9-1")),
            );
            match result {
                Err(PackageError::ClaimsMismatch { field: found, .. }) => assert_eq!(found, field),
                other => panic!("esperava ClaimsMismatch em {field}, veio {other:?}"),
            }
        }
    }

    #[test]
    fn another_architecture_is_refused_but_all_is_fine() {
        let arm = check_package(
            &claims("rustployd", "0.1.10-1", "arm64"),
            &facts("rustployd", "0.1.10-1", "arm64"),
            &expect(Some("0.1.9-1")),
        );
        assert!(matches!(arm, Err(PackageError::WrongArchitecture { .. })));

        let all = check_package(
            &claims("rustployd", "0.1.10-1", "all"),
            &facts("rustployd", "0.1.10-1", "all"),
            &expect(Some("0.1.9-1")),
        );
        assert_eq!(all, Ok(()));
    }

    #[test]
    fn same_or_older_version_is_a_refused_downgrade() {
        for installed in ["0.1.10-1", "0.1.11-1", "1:0.0.1-1"] {
            let result = check_package(
                &claims("rustployd", "0.1.10-1", "amd64"),
                &facts("rustployd", "0.1.10-1", "amd64"),
                &expect(Some(installed)),
            );
            assert!(
                matches!(result, Err(PackageError::NotNewer { .. })),
                "{installed}"
            );
        }
    }

    #[test]
    fn first_install_has_nothing_to_compare_but_must_be_a_valid_version() {
        let ok = check_package(
            &claims("rustployd", "0.1.10-1", "amd64"),
            &facts("rustployd", "0.1.10-1", "amd64"),
            &expect(None),
        );
        assert_eq!(ok, Ok(()));
        let bad = check_package(
            &claims("rustployd", "lixo", "amd64"),
            &facts("rustployd", "lixo", "amd64"),
            &expect(None),
        );
        assert!(matches!(bad, Err(PackageError::BadVersion(_))));
    }

    #[test]
    fn parses_dpkg_deb_output_in_any_order() {
        let parsed =
            parse_dpkg_fields("Version: 0.1.10-1\nPackage: rustployd\nArchitecture: amd64\n")
                .unwrap();
        assert_eq!(parsed, facts("rustployd", "0.1.10-1", "amd64"));
        assert_eq!(
            parse_dpkg_fields("Package: rustployd\n"),
            Err(PackageError::UnreadableFields)
        );
        assert_eq!(parse_dpkg_fields(""), Err(PackageError::UnreadableFields));
    }
}
