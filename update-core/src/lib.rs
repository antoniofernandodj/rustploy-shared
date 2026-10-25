//! Lógica de atualização comum ao daemon e ao app: um só lugar decide se uma
//! versão é mais nova que outra e se um pacote assinado pode ser instalado, para
//! as duas pontas nunca divergirem.
//!
//! - [`version`]: comparação de **tags** de release (semver, `v0.1.15`).
//! - [`dpkg_version`]: comparação de versões de **pacote** `.deb` (`0.1.15-1`), pela
//!   regra do dpkg. Em semver, o `-1` seria um pré-lançamento e `0.1.15-1` ficaria
//!   *menor* que `0.1.15`.
//! - [`signature`]: confere o `.minisig` e devolve o que o comentário assinado afirma.
//! - [`package`]: cruza assinatura, `.deb` e máquina e decide se pode instalar.
//! - [`staging`]: cópia segura do arquivo para um diretório só do root.
//! - [`outcome`]: o resultado da última atualização, lido pelo daemon ao reiniciar.

pub mod dpkg_version;
pub mod outcome;
pub mod package;
pub mod signature;
#[cfg(unix)]
pub mod staging;
mod version;

pub use version::{VersionError, is_newer, parse_version};
