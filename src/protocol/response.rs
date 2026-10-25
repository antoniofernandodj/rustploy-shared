//! `Response`: a resposta a cada `Command`.

use crate::domain::*;
use serde::{Deserialize, Serialize};

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Project(Box<Project>),
    Projects(Vec<Project>),
    Service(Box<Service>),
    Services(Vec<Service>),
    Deployment(Box<Deployment>),
    Deployments(Vec<Deployment>),
    Logs(Vec<LogEntry>),
    BuildLogs(Vec<BuildLogLine>),
    DeploymentSummaries(Vec<DeploymentSummary>),
    DaemonStatus(DaemonStatus),
    UpdateInfo(Box<UpdateInfo>),
    DeployEngineStatus(Box<DeployEngineSummary>),
    Pong {
        uptime_secs: u64,
    },
    WebhookUrl(Option<String>),
    SharedAccessList(Vec<SharedAccess>),
    Migration(Box<Migration>),
    Migrations(Vec<Migration>),
    ManagedDatabases(Vec<ManagedDatabase>),
    ConnectionInfo {
        /// Hostname que resolve na rede do projeto (chave do YAML, alias ou,
        /// sem certeza, o nome do container).
        host: String,
        /// URI pronta (`postgresql://user:senha@host:5432/db`).
        internal_url: String,
    },
    DaemonSettings {
        /// URL pública da API, **derivada** de `[api] domain`/`port` — base das
        /// URLs de webhook e do callback OAuth. Só-leitura: não existe setter,
        /// muda-se editando a config do daemon.
        public_base_url: String,
        acme_email: Option<String>,
        registry_domain: Option<String>,
    },
    SecretNames(Vec<String>),
    ManifestReport(ApplyReport),
    /// Manifesto YAML serializado (resposta de `ManifestExport`).
    Manifest(String),
    /// YAML + `.env` complementar (resposta de `ManifestExportAll`).
    ManifestBundle {
        yaml: String,
        dotenv: String,
    },
    /// Resposta de `ServiceExportPlan`.
    ServiceExportPlan(ServiceExportPlan),
    /// Resposta de `ServiceExport`: o pacote e um nome de arquivo sugerido.
    ServiceBundleYaml {
        yaml: String,
        filename: String,
    },
    /// Resposta de `ServiceImport` (inclusive o `dry_run`).
    ServiceImportReport(Box<ServiceImportReport>),
    /// `${VAR}` sem valor correspondente no `.env` (resposta de
    /// `ManifestImport` quando faltam variáveis) — nada foi aplicado.
    MissingEnvVars(Vec<String>),

    // Git providers
    GitProviders(Vec<GitProvider>),
    GitProviderInfo(Box<GitProvider>),
    /// Authorization URL the client should open (resposta de `GitOAuthStart`).
    OAuthUrl(String),
    GitRepos(Vec<GitRepo>),
    GitBranches(Vec<GitBranch>),

    PruneResult {
        count: u32,
        reclaimed_bytes: u64,
    },
    /// Resposta de `DockerCleanupConfigGet`/`Set` — `last_run` é `None`
    /// enquanto a limpeza automática nunca rodou.
    DockerCleanupConfig {
        config: DockerCleanupConfig,
        last_run: Option<DockerCleanupLastRun>,
    },
    EnvBackupSnapshots(Vec<String>),

    // Docker inventory
    DockerImages(Vec<DockerImageInfo>),
    DockerVolumes(Vec<DockerVolumeInfo>),
    DockerNetworks(Vec<DockerNetworkInfo>),
    /// Todo container do host (rodando + parado), resposta de `DockerContainers`.
    DockerContainers(Vec<DockerContainerInfo>),
    /// Count of rustploy-managed containers stopped (resposta de `StopAllManaged`).
    StopAllResult {
        count: u32,
    },

    /// Tabela de rotas viva do ingress (resposta de `IngressRoutes` e de
    /// `IngressReconcile` — o reconcile devolve a tabela já recalculada).
    IngressRoutes(IngressSnapshot),

    /// Catálogos do wizard, prontos como JSON para o contexto (`new_service_databases`,
    /// `new_service_brokers`, `new_service_templates`). Resposta de `WizardCatalog`.
    WizardCatalog {
        dbs: String,
        brokers: String,
        templates: String,
    },

    /// Snapshot do dashboard como JSON (resposta de `Snapshot`).
    Snapshot(String),

    // Jobs
    Job(Box<Job>),
    Jobs(Vec<Job>),
    JobSummaries(Vec<JobSummary>),
    JobRun(JobRun),
    JobRuns(Vec<JobRun>),
    /// Log de uma execução (resposta de `GetJobLogs`) — reaproveita
    /// `BuildLogLine`, mesma forma de linha (stream + texto + timestamp).
    JobLogs(Vec<BuildLogLine>),
    /// Resposta de `JobWarmStatus`.
    JobWarmStatus(JobWarmStatus),
    /// Resposta de `JobWarm`: `already_warm` = nada foi disparado porque o job
    /// já estava quente.
    JobWarmRequested {
        already_warm: bool,
    },
    /// Resposta de `ServiceWarmStatusList`.
    ServiceWarmStatuses(Vec<ServiceWarmStatus>),
    /// Resposta de `ServiceWarm`: `already_warm` = nada foi disparado porque o
    /// serviço já estava quente.
    ServiceWarmRequested {
        already_warm: bool,
    },

    // Registry OCI embutido
    RegistryStatus(RegistryStatusInfo),
    RegistryRepos(Vec<RegistryRepoInfo>),
    RegistryTags(Vec<RegistryTagInfo>),
    /// Resultado do `RegistryGc`: arquivos removidos do CAS/uploads e bytes
    /// liberados no disco.
    RegistryGcResult {
        blobs_removed: u64,
        bytes_freed: u64,
    },
    /// Resposta de `RegistryTokenCreate` — `secret` só aparece aqui, uma vez.
    RegistryTokenCreated {
        name: String,
        secret: String,
    },
    RegistryTokens(Vec<RegistryTokenInfo>),

    Err {
        code: String,
        message: String,
    },
}

impl Response {
    pub fn err(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Err {
            code: code.into(),
            message: message.into(),
        }
    }
}
