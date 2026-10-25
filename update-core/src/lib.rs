//! Lógica de atualização comum ao daemon e ao app: um só lugar decide se uma
//! versão é mais nova que outra, para as duas pontas nunca divergirem.
//!
//! Este crate só entende **tags de release** (`v0.1.15`, semver). A versão de um
//! pacote `.deb` (`0.1.15-1`) segue a regra do dpkg e é comparada com
//! `dpkg --compare-versions` na hora de instalar, não aqui: em semver, o `-1`
//! seria um pré-lançamento e `0.1.15-1` ficaria *menor* que `0.1.15`.

mod version;

pub use version::{VersionError, is_newer, parse_version};
