//! Comparação de versões de **pacote Debian** (`1:0.1.10-1`), pelo algoritmo da
//! política Debian (o mesmo do `dpkg --compare-versions`).
//!
//! Serve para a regra "sem downgrade" do updater: instalar só o que for
//! estritamente mais novo que o instalado. Não confundir com `version.rs`, que
//! compara *tags* (semver): em semver `0.1.10-1` seria um pré-lançamento e ficaria
//! menor que `0.1.10`; no dpkg o `-1` é a revisão do pacote e ele fica maior.

use std::{cmp::Ordering, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpkgVersionError {
    input: String,
}

impl fmt::Display for DpkgVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "versão de pacote inválida: {:?}", self.input)
    }
}

impl std::error::Error for DpkgVersionError {}

struct Parts<'a> {
    epoch: u64,
    upstream: &'a str,
    revision: &'a str,
}

fn split(version: &str) -> Result<Parts<'_>, DpkgVersionError> {
    let error = || DpkgVersionError {
        input: version.to_string(),
    };
    let version = version.trim();
    let (epoch, rest) = match version.split_once(':') {
        Some((epoch, rest)) => (epoch.parse::<u64>().map_err(|_| error())?, rest),
        None => (0, version),
    };
    let (upstream, revision) = match rest.rsplit_once('-') {
        Some((upstream, revision)) => (upstream, revision),
        None => (rest, ""),
    };
    // A política exige que a versão do upstream comece por dígito.
    if !upstream.starts_with(|character: char| character.is_ascii_digit()) {
        return Err(error());
    }
    let allowed =
        |character: char| character.is_ascii_alphanumeric() || ".+~-:".contains(character);
    if !upstream.chars().all(allowed) || !revision.chars().all(allowed) {
        return Err(error());
    }
    Ok(Parts {
        epoch,
        upstream,
        revision,
    })
}

/// Peso de um caractere que não é dígito: `~` vem antes de tudo (até do fim da
/// string), letras vêm antes de pontuação.
fn order(byte: Option<u8>) -> i32 {
    match byte {
        None => 0,
        Some(byte) if byte.is_ascii_digit() => 0,
        Some(byte) if byte.is_ascii_alphabetic() => i32::from(byte),
        Some(b'~') => -1,
        Some(byte) => i32::from(byte) + 256,
    }
}

fn compare_part(left: &str, right: &str) -> Ordering {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < left.len() || j < right.len() {
        let mut first_difference = 0i32;
        let is_digit = |bytes: &[u8], at: usize| bytes.get(at).is_some_and(u8::is_ascii_digit);
        // Trecho não numérico: compara caractere a caractere pelo peso.
        while (i < left.len() && !is_digit(left, i)) || (j < right.len() && !is_digit(right, j)) {
            let left_weight = order(left.get(i).copied());
            let right_weight = order(right.get(j).copied());
            if left_weight != right_weight {
                return left_weight.cmp(&right_weight);
            }
            i += 1;
            j += 1;
        }
        // Trecho numérico: ignora zeros à esquerda e compara por tamanho e depois por dígito.
        while left.get(i) == Some(&b'0') {
            i += 1;
        }
        while right.get(j) == Some(&b'0') {
            j += 1;
        }
        while is_digit(left, i) && is_digit(right, j) {
            if first_difference == 0 {
                first_difference = i32::from(left[i]) - i32::from(right[j]);
            }
            i += 1;
            j += 1;
        }
        if is_digit(left, i) {
            return Ordering::Greater;
        }
        if is_digit(right, j) {
            return Ordering::Less;
        }
        if first_difference != 0 {
            return first_difference.cmp(&0);
        }
    }
    Ordering::Equal
}

/// Compara duas versões de pacote Debian (época, depois upstream, depois revisão).
pub fn compare(left: &str, right: &str) -> Result<Ordering, DpkgVersionError> {
    let (left, right) = (split(left)?, split(right)?);
    Ok(left
        .epoch
        .cmp(&right.epoch)
        .then_with(|| compare_part(left.upstream, right.upstream))
        .then_with(|| compare_part(left.revision, right.revision)))
}

/// `true` se `candidate` é **estritamente** mais nova que `installed`.
pub fn is_newer(candidate: &str, installed: &str) -> Result<bool, DpkgVersionError> {
    Ok(compare(candidate, installed)? == Ordering::Greater)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pares `(a, b)` em que `a < b` (casos clássicos da política Debian).
    const ASCENDING: &[(&str, &str)] = &[
        ("0.1.9-1", "0.1.10-1"),
        ("0.1.10-1", "0.1.10-2"),
        ("0.1.10-1", "0.2.0-1"),
        ("0.1.10", "0.1.10-1"),
        ("1.0~rc1-1", "1.0-1"),
        ("1.0~~", "1.0~"),
        ("1.0~", "1.0"),
        ("1.0", "1.0+1"),
        ("1.0a", "1.0+"),
        ("1.9", "1.10"),
        ("1.002", "1.3"),
        ("0:9.9-1", "1:0.1-1"),
        ("1.0-1", "1.0-1+b1"),
        ("2.0", "2.0.1"),
        ("1.0a-1", "1.0b-1"),
    ];

    #[test]
    fn ordering_follows_the_debian_policy() {
        for (smaller, larger) in ASCENDING {
            assert_eq!(
                compare(smaller, larger).unwrap(),
                Ordering::Less,
                "{smaller} < {larger}"
            );
            assert_eq!(
                compare(larger, smaller).unwrap(),
                Ordering::Greater,
                "{larger} > {smaller}"
            );
        }
    }

    #[test]
    fn equal_versions_ignore_leading_zeros_and_empty_revision() {
        assert_eq!(compare("1.0-1", "1.0-1").unwrap(), Ordering::Equal);
        assert_eq!(compare("1.01", "1.1").unwrap(), Ordering::Equal);
        assert_eq!(compare("1.0", "1.0-0").unwrap(), Ordering::Equal);
        assert_eq!(compare("0:1.0", "1.0").unwrap(), Ordering::Equal);
    }

    #[test]
    fn is_newer_is_strict() {
        assert!(is_newer("0.1.10-1", "0.1.9-1").unwrap());
        assert!(!is_newer("0.1.10-1", "0.1.10-1").unwrap());
        assert!(!is_newer("0.1.9-1", "0.1.10-1").unwrap());
    }

    #[test]
    fn rejects_what_is_not_a_package_version() {
        for bad in [
            "", "abc", "x1.0", "1:", ":1.0", "a:1.0", "1.0 beta", "1.0_1", "-1",
        ] {
            assert!(compare(bad, "1.0").is_err(), "{bad:?} deveria falhar");
        }
    }

    /// O dpkg de verdade é a referência: o resultado daqui tem de bater com ele,
    /// para uma tabela de pares maior que a acima. Sem `dpkg` na máquina, não roda.
    #[test]
    fn agrees_with_the_real_dpkg() {
        let Ok(probe) = std::process::Command::new("dpkg").arg("--version").output() else {
            eprintln!("sem dpkg: teste pulado");
            return;
        };
        if !probe.status.success() {
            return;
        }
        let versions = [
            "0.1.9-1",
            "0.1.10-1",
            "0.1.10-2",
            "0.1.10",
            "0.2.0-1",
            "1.0~rc1-1",
            "1.0-1",
            "1.0~",
            "1.0",
            "1.0+1",
            "1.9",
            "1.10",
            "1.002",
            "1.3",
            "0:9.9-1",
            "1:0.1-1",
            "1.0-1+b1",
            "2.0",
            "2.0.1",
            "1.0a-1",
            "1.0b-1",
            "1.0~~",
            "3.0-0ubuntu1",
            "3.0-0ubuntu1~1",
        ];
        for left in versions {
            for right in versions {
                let expected = ["lt", "eq", "gt"].into_iter().find(|operator| {
                    std::process::Command::new("dpkg")
                        .args(["--compare-versions", left, operator, right])
                        .status()
                        .map(|status| status.success())
                        .unwrap_or(false)
                });
                let ours = match compare(left, right).unwrap() {
                    Ordering::Less => "lt",
                    Ordering::Equal => "eq",
                    Ordering::Greater => "gt",
                };
                assert_eq!(
                    expected,
                    Some(ours),
                    "dpkg e update-core divergem em {left} vs {right}"
                );
            }
        }
    }
}
