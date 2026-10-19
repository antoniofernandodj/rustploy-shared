use serde::{Deserialize, Serialize};

/// Resultado de um `apply`: o que foi criado/atualizado/removido em cada recurso,
/// mais a lista de serviços para os quais um deploy foi disparado (`--deploy`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApplyReport {
    pub actions: Vec<ResourceAction>,
    pub deployed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAction {
    pub kind: ResourceActionKind,
    /// `nome-do-projeto` ou `projeto/serviço`.
    pub name: String,
    pub action: ActionVerb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceActionKind {
    Project,
    Service,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionVerb {
    Created,
    Updated,
    Unchanged,
    Deleted,
}

impl std::fmt::Display for ActionVerb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Updated => write!(f, "updated"),
            Self::Unchanged => write!(f, "unchanged"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

impl std::fmt::Display for ResourceActionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project => write!(f, "project"),
            Self::Service => write!(f, "service"),
        }
    }
}
