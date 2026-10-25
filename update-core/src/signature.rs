//! Verificação da assinatura minisign de um pacote e leitura do comentário confiável.
//!
//! O comentário confiável entra na assinatura (não dá para trocá-lo sem a chave
//! privada) e carrega `package=<nome> version=<versão> arch=<arquitetura>`. Quem
//! instala confere esses campos com os do próprio `.deb` (`package::check_package`).

use minisign_verify::{PublicKey, Signature};
use std::fmt;

/// Chaves públicas em que os updaters confiam: a atual e, na hora de rotacionar,
/// a próxima (uma versão que já confia na nova é publicada **antes** de a antiga
/// sair). Fonte: `keys/rustploy-update.pub` no agregador.
pub const TRUSTED_PUBLIC_KEYS: &[&str] =
    &["RWSWAQvUAwKjxh3dDwqvzjm9XPx/tEqDqrctDWMlwA22TgSzy/0V2kP/"];

/// O que a assinatura afirma sobre o arquivo assinado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedClaims {
    pub package: String,
    pub version: String,
    pub arch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureError {
    /// Nenhuma chave de confiança foi dada (configuração do updater errada).
    NoTrustedKeys,
    /// Uma das chaves embutidas não decodifica: bug do updater, não do pacote.
    BadTrustedKey(String),
    /// O texto do `.minisig` não é uma assinatura minisign.
    MalformedSignature(String),
    /// Nenhuma chave de confiança assina este arquivo (adulterado, ou de outra chave).
    NotTrusted,
    /// A assinatura é válida, mas o comentário confiável não traz `package`, `version` e `arch`.
    MissingClaims(String),
}

impl fmt::Display for SignatureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTrustedKeys => write!(formatter, "nenhuma chave de confiança configurada"),
            Self::BadTrustedKey(detail) => {
                write!(formatter, "chave de confiança inválida: {detail}")
            }
            Self::MalformedSignature(detail) => write!(formatter, "assinatura ilegível: {detail}"),
            Self::NotTrusted => write!(
                formatter,
                "assinatura inválida: o arquivo não foi assinado por uma chave de confiança"
            ),
            Self::MissingClaims(comment) => write!(
                formatter,
                "o comentário assinado não diz package, version e arch: {comment:?}"
            ),
        }
    }
}

impl std::error::Error for SignatureError {}

/// Confere `data` contra `signature_text` (o conteúdo do `.minisig`) usando as
/// chaves dadas e devolve o que o comentário confiável afirma.
pub fn verify(
    data: &[u8],
    signature_text: &str,
    public_keys: &[&str],
) -> Result<SignedClaims, SignatureError> {
    if public_keys.is_empty() {
        return Err(SignatureError::NoTrustedKeys);
    }
    let signature = Signature::decode(signature_text)
        .map_err(|error| SignatureError::MalformedSignature(error.to_string()))?;

    for key_text in public_keys {
        let key = PublicKey::from_base64(key_text)
            .map_err(|error| SignatureError::BadTrustedKey(error.to_string()))?;
        // `false`: só a assinatura moderna (pré-hash), nunca o formato legado.
        if key.verify(data, &signature, false).is_ok() {
            return parse_claims(signature.trusted_comment());
        }
    }
    Err(SignatureError::NotTrusted)
}

fn parse_claims(trusted_comment: &str) -> Result<SignedClaims, SignatureError> {
    let (mut package, mut version, mut arch) = (None, None, None);
    for pair in trusted_comment.split_whitespace() {
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        let slot = match name {
            "package" => &mut package,
            "version" => &mut version,
            "arch" => &mut arch,
            _ => continue,
        };
        // Campo repetido seria ambíguo: recusa em vez de escolher um.
        if slot.replace(value.to_string()).is_some() {
            return Err(SignatureError::MissingClaims(trusted_comment.to_string()));
        }
    }
    match (package, version, arch) {
        (Some(package), Some(version), Some(arch)) if !package.is_empty() => Ok(SignedClaims {
            package,
            version,
            arch,
        }),
        _ => Err(SignatureError::MissingClaims(trusted_comment.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Par de chaves DESCARTÁVEL só para os testes (sem senha), gerado com
    // `minisign -G -W`. Nunca é a chave de produção.
    const TEST_PUBLIC_KEY: &str = include_str!("../tests/fixtures/test.pub");
    const OTHER_PUBLIC_KEY: &str = include_str!("../tests/fixtures/other.pub");
    const PAYLOAD: &[u8] = include_bytes!("../tests/fixtures/payload.bin");
    const SIGNATURE: &str = include_str!("../tests/fixtures/payload.bin.minisig");
    const NO_CLAIMS_SIGNATURE: &str = include_str!("../tests/fixtures/payload.noclaims.minisig");

    fn key_line(file_contents: &str) -> &str {
        file_contents.lines().nth(1).unwrap().trim()
    }

    #[test]
    fn valid_signature_returns_the_signed_claims() {
        let claims = verify(PAYLOAD, SIGNATURE, &[key_line(TEST_PUBLIC_KEY)]).unwrap();
        assert_eq!(
            claims,
            SignedClaims {
                package: "rustployd".into(),
                version: "0.2.0-1".into(),
                arch: "amd64".into(),
            }
        );
    }

    #[test]
    fn tampered_data_is_not_trusted() {
        let mut tampered = PAYLOAD.to_vec();
        tampered.push(b'x');
        assert_eq!(
            verify(&tampered, SIGNATURE, &[key_line(TEST_PUBLIC_KEY)]),
            Err(SignatureError::NotTrusted)
        );
    }

    #[test]
    fn a_key_that_did_not_sign_is_not_trusted() {
        assert_eq!(
            verify(PAYLOAD, SIGNATURE, &[key_line(OTHER_PUBLIC_KEY)]),
            Err(SignatureError::NotTrusted)
        );
    }

    #[test]
    fn any_of_the_trusted_keys_may_have_signed() {
        // Rotação: a chave nova e a antiga convivem durante a troca.
        let keys = [key_line(OTHER_PUBLIC_KEY), key_line(TEST_PUBLIC_KEY)];
        assert!(verify(PAYLOAD, SIGNATURE, &keys).is_ok());
    }

    #[test]
    fn tampered_trusted_comment_is_not_trusted() {
        // Troca a versão afirmada pela assinatura: a assinatura global não bate.
        let forged = SIGNATURE.replace("version=0.2.0-1", "version=9.9.9-1");
        assert_ne!(forged, SIGNATURE, "o fixture mudou e o teste ficou vazio");
        assert_eq!(
            verify(PAYLOAD, &forged, &[key_line(TEST_PUBLIC_KEY)]),
            Err(SignatureError::NotTrusted)
        );
    }

    #[test]
    fn signature_without_claims_is_refused() {
        assert!(matches!(
            verify(PAYLOAD, NO_CLAIMS_SIGNATURE, &[key_line(TEST_PUBLIC_KEY)]),
            Err(SignatureError::MissingClaims(_))
        ));
    }

    #[test]
    fn garbage_signature_and_missing_keys_are_distinct_errors() {
        assert!(matches!(
            verify(
                PAYLOAD,
                "isto não é uma assinatura",
                &[key_line(TEST_PUBLIC_KEY)]
            ),
            Err(SignatureError::MalformedSignature(_))
        ));
        assert_eq!(
            verify(PAYLOAD, SIGNATURE, &[]),
            Err(SignatureError::NoTrustedKeys)
        );
        assert!(matches!(
            verify(PAYLOAD, SIGNATURE, &["não é uma chave"]),
            Err(SignatureError::BadTrustedKey(_))
        ));
    }

    /// Confere uma release **real** com a chave embutida. Não roda sozinho (precisa dos
    /// arquivos): `RUSTPLOY_REAL_DEB=<.deb> RUSTPLOY_REAL_SIG=<.minisig> cargo test -p
    /// rustploy-update-core real_release -- --ignored`. Rode depois de cada release.
    #[test]
    #[ignore = "precisa dos arquivos de uma release real (variáveis de ambiente)"]
    fn real_release_verifies_with_the_embedded_key() {
        let deb = std::env::var("RUSTPLOY_REAL_DEB").expect("RUSTPLOY_REAL_DEB");
        let sig = std::env::var("RUSTPLOY_REAL_SIG").expect("RUSTPLOY_REAL_SIG");
        let claims = verify(
            &std::fs::read(deb).unwrap(),
            &std::fs::read_to_string(sig).unwrap(),
            TRUSTED_PUBLIC_KEYS,
        )
        .unwrap();
        eprintln!("assinado: {claims:?}");
        assert!(!claims.package.is_empty());
    }

    #[test]
    fn the_embedded_production_key_decodes() {
        for key in TRUSTED_PUBLIC_KEYS {
            assert!(PublicKey::from_base64(key).is_ok(), "{key}");
        }
    }

    #[test]
    fn claims_parsing_is_strict_about_repeats_and_blanks() {
        assert_eq!(
            parse_claims("package=a version=1 arch=amd64")
                .unwrap()
                .package,
            "a"
        );
        assert!(parse_claims("package=a package=b version=1 arch=amd64").is_err());
        assert!(parse_claims("package=a version=1").is_err());
        assert!(parse_claims("package= version=1 arch=amd64").is_err());
        assert!(parse_claims("").is_err());
        // Campos extras são ignorados (compatibilidade futura).
        assert!(parse_claims("package=a version=1 arch=amd64 build=7").is_ok());
    }
}
