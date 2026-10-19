//! Tipos compartilhados entre o daemon e os clientes: domínio, protocolo e nomes derivados.

pub mod domain;
pub mod naming;
pub mod protocol;

/// Alias de [`domain`], o nome antigo do módulo de modelos.
pub mod models {
    pub use crate::domain::*;
}

pub use domain::*;
pub use naming::*;
pub use protocol::{Command, Event, Response};
