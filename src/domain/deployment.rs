use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Deployment {
    pub id: String,
    pub service_id: String,
    pub image: String,
    pub state: DeployState,
    pub states_log: Vec<StateTransition>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeployState {
    Pending,
    /// Roda a fila `ServiceSpec.pre_deploy_checks()` em ordem (quando não
    /// vazia) e só avança para `ResolvingDeps` se TODOS passarem (exit code
    /// 0) — a primeira falha interrompe a fila e o deploy. Fila vazia passa
    /// direto — no-op.
    PreDeployCheck,
    ResolvingDeps,
    PullingImage,
    CloningRepo,
    BuildingImage,
    Staging,
    HealthcheckPolling,
    SwappingIn,
    Draining,
    Promoting,
    Live,
    Stopped,
    RollingBack,
    Failed,
    Pruning,
    ComposingUp,
}

impl DeployState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Live | Self::Stopped | Self::Failed | Self::Pruning
        )
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::PreDeployCheck => "PreDeployCheck",
            Self::ResolvingDeps => "ResolvingDeps",
            Self::PullingImage => "PullingImage",
            Self::CloningRepo => "CloningRepo",
            Self::BuildingImage => "BuildingImage",
            Self::Staging => "Staging",
            Self::HealthcheckPolling => "HealthcheckPolling",
            Self::SwappingIn => "SwappingIn",
            Self::Draining => "Draining",
            Self::Promoting => "Promoting",
            Self::Live => "Live",
            Self::Stopped => "Stopped",
            Self::RollingBack => "RollingBack",
            Self::Failed => "Failed",
            Self::Pruning => "Pruning",
            Self::ComposingUp => "ComposingUp",
        }
    }
}

#[cfg(test)]
mod pre_deploy_gate_tests {
    use super::*;
    use crate::domain::*;

    fn sample_spec(
        pre_deploy_job_id: Option<String>,
        pre_deploy_job_ids: Vec<String>,
    ) -> ServiceSpec {
        ServiceSpec {
            name: "svc".into(),
            project_id: "proj-1".into(),
            source: ServiceSource::Registry {
                image: "nginx:latest".into(),
            },
            port: 8080,
            host_port: None,
            domain: None,
            tls_enabled: false,
            env_vars: vec![],
            env_comments: vec![],
            volumes: vec![],
            healthcheck: Healthcheck::default(),
            replicas: 1,
            resources: ResourceLimits::default(),
            run_command: None,
            run_args: vec![],
            db_kind: None,
            domains: vec![],
            pre_deploy_job_id,
            pre_deploy_job_ids,
            shared: None,
        }
    }

    #[test]
    fn pre_deploy_job_ids_json_round_trip() {
        let spec = sample_spec(
            None,
            vec!["job_migration".into(), "job_lint".into(), "job_test".into()],
        );
        let json = serde_json::to_string(&spec).unwrap();
        let back: ServiceSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.pre_deploy_job_ids,
            vec![
                "job_migration".to_string(),
                "job_lint".to_string(),
                "job_test".to_string()
            ]
        );
    }

    #[test]
    fn pre_deploy_job_ids_back_compat_when_absent() {
        // Specs salvos antes da feature não têm o campo; serde default
        // precisa preencher como vec vazio (mesmo padrão de domains/etc.).
        let spec = sample_spec(None, vec![]);
        let mut value = serde_json::to_value(&spec).unwrap();
        value.as_object_mut().unwrap().remove("pre_deploy_job_ids");
        let back: ServiceSpec = serde_json::from_value(value).unwrap();
        assert_eq!(back.pre_deploy_job_ids, Vec::<String>::new());
    }

    #[test]
    fn pre_deploy_checks_falls_back_to_legacy_single_job() {
        let spec = sample_spec(Some("job_legacy".into()), vec![]);
        assert_eq!(spec.pre_deploy_checks(), vec!["job_legacy".to_string()]);
    }

    #[test]
    fn pre_deploy_checks_prefers_queue_over_legacy() {
        let spec = sample_spec(
            Some("job_legacy".into()),
            vec!["job_a".into(), "job_b".into()],
        );
        assert_eq!(
            spec.pre_deploy_checks(),
            vec!["job_a".to_string(), "job_b".to_string()]
        );
    }

    #[test]
    fn materialize_pre_deploy_checks_moves_legacy_and_clears_it() {
        let mut spec = sample_spec(Some("job_legacy".into()), vec![]);
        spec.materialize_pre_deploy_checks();
        assert_eq!(spec.pre_deploy_job_ids, vec!["job_legacy".to_string()]);
        assert_eq!(spec.pre_deploy_job_id, None);
    }

    #[test]
    fn deploy_state_pre_deploy_check_label_and_serde() {
        assert_eq!(DeployState::PreDeployCheck.label(), "PreDeployCheck");
        let json = serde_json::to_string(&DeployState::PreDeployCheck).unwrap();
        assert_eq!(json, "\"PreDeployCheck\"");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateTransition {
    pub from: DeployState,
    pub to: DeployState,
    pub at: DateTime<Utc>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentSummary {
    pub deployment: Deployment,
    pub service_name: String,
    pub project_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub version: String,
    pub uptime_secs: u64,
    pub services_running: usize,
    pub services_total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveDeployInfo {
    pub deployment_id: String,
    pub service_id: String,
    pub service_name: String,
    pub project_name: String,
    pub state: DeployState,
    pub started_at: DateTime<Utc>,
    pub elapsed_secs: u64,
    pub current_state_secs: u64,
    /// Histórico de transições do deploy (o `states_log`), em ordem. Alimenta o
    /// stepper detalhado dos clientes; daemons antigos não o enviam.
    #[serde(default)]
    pub states: Vec<StateTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployEngineSummary {
    pub version: String,
    pub uptime_secs: u64,
    pub active: Vec<ActiveDeployInfo>,
    pub recent: Vec<ActiveDeployInfo>,
    pub total_24h: u64,
    pub successful_24h: u64,
    pub failed_24h: u64,
    /// Deploys esperando na fila global (em ordem de execução; o primeiro é o
    /// próximo a rodar).
    pub queued: Vec<ActiveDeployInfo>,
    /// Fila pausada — o worker não puxa o próximo até retomar.
    pub paused: bool,
}
