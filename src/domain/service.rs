use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::*;
use crate::naming::normalize_name;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceSpec {
    pub name: String,
    pub project_id: String,
    pub source: ServiceSource,
    pub port: u16,
    #[serde(default)]
    pub host_port: Option<u16>,
    pub domain: Option<String>,
    #[serde(default)]
    pub tls_enabled: bool,
    pub env_vars: Vec<EnvVar>,
    /// Comentários (`# ...`) do editor `.env`, ancorados por `key` (posição
    /// preservada mesmo que as vars sejam reordenadas). Não participa da
    /// construção do ambiente do container — só a lista/edição via UI olha
    /// pra isso.
    #[serde(default)]
    pub env_comments: Vec<EnvComment>,
    pub volumes: Vec<VolumeMount>,
    pub healthcheck: Healthcheck,
    pub replicas: u32,
    pub resources: ResourceLimits,
    #[serde(default)]
    pub run_command: Option<String>,
    #[serde(default)]
    pub run_args: Vec<String>,
    /// Tipo de serviço gerenciado — banco (postgres | mongodb | mariadb |
    /// mysql | redis) ou broker (kafka | rabbitmq | nats). Controla a aba
    /// Connection no painel e a geração da internal connection URL (scheme por
    /// tipo: postgres://, amqp://, nats://, Kafka sem scheme).
    #[serde(default)]
    pub db_kind: Option<String>,
    /// Rotas HTTP de domínio. Cada domínio pode apontar para uma porta de
    /// container diferente e ligar TLS de forma independente — permite um
    /// serviço que responde em várias portas expor vários subdomínios.
    /// Retrocompat: quando vazio, cai no `domain`/`tls_enabled` legado (roteado
    /// para `port`). Ver [`ServiceSpec::domain_routes`].
    #[serde(default)]
    pub domains: Vec<DomainRoute>,
    /// Legado (pré-fila, até 2026-08-05): um único `Job` de pré-deploy check.
    /// Substituído por `pre_deploy_job_ids`; mantido só pra retrocompat de
    /// specs salvos antes — nunca mais escrito por código novo. Ver
    /// [`ServiceSpec::pre_deploy_checks`].
    #[serde(default)]
    pub pre_deploy_job_id: Option<String>,
    /// Fila de `Job`s (Schedules) rodados em ORDEM antes de tocar em
    /// qualquer coisa (pull/build/staging) — vazio = sem checks, deploy
    /// segue direto. Cada item roda até completar (exit code do
    /// `main_service`); a primeira falha interrompe a fila **e** o deploy
    /// inteiro, sem rodar os checks seguintes. Ver `DeployState::
    /// PreDeployCheck` e `docs/plano-pre-deploy-gate.md`. Retrocompat: quando
    /// vazio, cai no `pre_deploy_job_id` legado — ver
    /// [`ServiceSpec::pre_deploy_checks`].
    #[serde(default)]
    pub pre_deploy_job_ids: Vec<String>,
    /// Se `Some`, este serviço de banco é um **servidor compartilhado**: o
    /// daemon o conecta às redes dos projetos autorizados (`shared_access`),
    /// sob o alias global [`shared_alias`](crate::shared_alias). Só vale com
    /// `db_kind` de banco. Ver `docs/plano-banco-compartilhado.md` §2.3/§4.1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<SharedServerConfig>,
}

/// Configuração de um servidor de banco compartilhado. Hoje só marca o serviço
/// como tal; é struct (e não `bool`) para receber opções sem quebrar o spec.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SharedServerConfig {}

/// Uma rota HTTP de domínio de um serviço: qual domínio, para qual porta do
/// container e com ou sem TLS.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainRoute {
    pub domain: String,
    /// Porta do container que este domínio atende. `None` = a `port` padrão do
    /// serviço.
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub tls: bool,
}

impl DomainRoute {
    /// Porta de container efetiva (a própria, ou a `port` padrão do serviço).
    pub fn container_port(&self, default: u16) -> u16 {
        self.port.unwrap_or(default)
    }
}

impl Service {
    /// Nome da stack Compose: o gravado; na falta (banco ainda não migrado, ou
    /// serviço que virou Compose depois), a fórmula legada com o nome atual —
    /// exatamente o que o daemon usava antes de o nome ser gravado.
    pub fn compose_project_name(&self) -> String {
        self.compose_project
            .clone()
            .unwrap_or_else(|| crate::compose_project_name(&self.id, &self.spec.name))
    }
}

impl ServiceSpec {
    pub fn safe_name(&self) -> String {
        normalize_name(&self.name)
    }

    /// Rotas HTTP efetivas do serviço: a lista `domains` nova, ou — para specs
    /// antigos que só têm o campo `domain` — uma rota única sintetizada a partir
    /// do domínio legado (roteada para `port`, TLS = `tls_enabled`).
    pub fn domain_routes(&self) -> Vec<DomainRoute> {
        if !self.domains.is_empty() {
            self.domains.clone()
        } else if let Some(d) = &self.domain {
            vec![DomainRoute {
                domain: d.clone(),
                port: None,
                tls: self.tls_enabled,
            }]
        } else {
            Vec::new()
        }
    }

    /// Portas de container que o ingress precisa alcançar: a de cada domínio,
    /// mais a `port` do serviço quando há rota de porta no host. Ordenadas e
    /// sem repetição.
    ///
    /// Serve para escolher, numa stack Compose, qual container recebe a rota
    /// (ver `docker::containers::find_compose_ingress_container`).
    pub fn ingress_container_ports(&self) -> Vec<u16> {
        let mut ports: Vec<u16> = self
            .domain_routes()
            .iter()
            .map(|r| r.container_port(self.port))
            .collect();
        if self.host_port.is_some() {
            ports.push(self.port);
        }
        ports.sort_unstable();
        ports.dedup();
        ports
    }

    /// Move o domínio legado (`domain`/`tls_enabled`) para a lista `domains` e
    /// zera os campos legados, para que edições subsequentes operem numa única
    /// fonte de verdade (a lista). Idempotente.
    pub fn materialize_domains(&mut self) {
        if self.domains.is_empty() {
            self.domains = self.domain_routes();
        }
        self.domain = None;
        self.tls_enabled = false;
    }

    /// Fila efetiva de checks de pré-deploy: `pre_deploy_job_ids` quando não
    /// vazia, ou — para specs antigos que só têm o campo singular — uma fila
    /// de um item sintetizada a partir do `pre_deploy_job_id` legado.
    pub fn pre_deploy_checks(&self) -> Vec<String> {
        if !self.pre_deploy_job_ids.is_empty() {
            self.pre_deploy_job_ids.clone()
        } else if let Some(j) = &self.pre_deploy_job_id {
            vec![j.clone()]
        } else {
            Vec::new()
        }
    }

    /// Move o `pre_deploy_job_id` legado para `pre_deploy_job_ids` e zera o
    /// campo legado — mesmo idioma de `materialize_domains`. Idempotente.
    pub fn materialize_pre_deploy_checks(&mut self) {
        if self.pre_deploy_job_ids.is_empty() {
            self.pre_deploy_job_ids = self.pre_deploy_checks();
        }
        self.pre_deploy_job_id = None;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Service {
    pub id: String,
    pub spec: ServiceSpec,
    /// Nome da stack Docker Compose (só serviços Compose), **gravado** na
    /// criação e nunca recalculado — renomear o serviço não muda a stack, então
    /// os volumes (`<stack>_<volume>`) continuam os mesmos. Só leitura para os
    /// clientes: vive numa coluna própria, fora do `spec`, que a GUI e a webui
    /// devolvem no `ServiceUpdate`. `None` em serviço que não é Compose; use
    /// [`Service::compose_project_name`].
    #[serde(default)]
    pub compose_project: Option<String>,
    pub status: ServiceStatus,
    pub live_container_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServiceStatus {
    Stopped,
    Stopping,
    Deploying,
    Running,
    Degraded,
    Error(String),
    /// Deploy criado e esperando na fila global — ainda não começou a rodar.
    Queued,
}

impl std::fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "Stopped"),
            Self::Stopping => write!(f, "Stopping"),
            Self::Deploying => write!(f, "Deploying"),
            Self::Running => write!(f, "Running"),
            Self::Degraded => write!(f, "Degraded"),
            Self::Error(msg) => write!(f, "Error: {msg}"),
            Self::Queued => write!(f, "Queued"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VolumeMount {
    pub host_path: String,
    pub container_path: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Healthcheck {
    pub kind: HealthcheckKind,
    pub interval_secs: u32,
    pub timeout_secs: u32,
    pub retries: u32,
    pub start_period_secs: u32,
}

impl Default for Healthcheck {
    fn default() -> Self {
        Self {
            // `None` (não `Tcp`): serviço novo nasce SEM verificação. Era `Tcp`
            // aqui e `None` no caminho de criação da webui, então o mesmo
            // "criar serviço" dava healthchecks diferentes conforme o cliente e
            // o caminho (wizard × `ServiceCreate` direto). Escolhido `None`
            // porque a porta declarada nem sempre é uma porta que aceita
            // conexão — worker e job não escutam nada, e um `Tcp` padrão
            // reprovaria o deploy deles. O custo é o deploy passar mesmo com a
            // porta errada; por isso o log do deploy agora diz, em letras
            // claras, que nada foi testado (ver `deploy::executor`).
            kind: HealthcheckKind::None,
            interval_secs: 5,
            timeout_secs: 3,
            retries: 10,
            start_period_secs: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthcheckKind {
    None,
    Http { path: String, expected_status: u16 },
    Tcp,
    DockerNative,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ResourceLimits {
    pub cpu_shares: u64,
    pub mem_limit_bytes: u64,
}

/// Se a imagem de um serviço com fonte Git ou zip já tem o cache de build quente
/// no host, de modo que o próximo deploy não baixe nem recompile tudo do zero.
/// Fonte única da verdade (`Command::ServiceWarmStatusList`): as telas só exibem
/// este valor, nunca o deduzem. Serviços de outras fontes (imagem pronta, etc.)
/// não têm build e não aparecem.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ServiceWarmStatus {
    pub service_id: String,
    pub warm: bool,
    /// Há um aquecimento em andamento (ou esperando a vaga da fila) agora.
    pub warming: bool,
    /// Por que está frio (`warm == false`): nunca aquecido, imagem ausente,
    /// cache de build limpo, fonte alterada. `None` quando quente.
    pub reason: Option<String>,
    /// Quando o serviço foi aquecido pela última vez (aquecimento explícito ou
    /// deploy bem-sucedido).
    pub warmed_at: Option<DateTime<Utc>>,
}
