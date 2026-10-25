//! `Command`: tudo o que o cliente pede ao daemon.

use crate::domain::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    // Projects
    ProjectCreate {
        name: String,
        description: Option<String>,
    },
    ProjectDelete {
        id: String,
    },
    ProjectUpdate {
        id: String,
        name: String,
        description: Option<String>,
    },
    ProjectList,
    ProjectEnvSet {
        project_id: String,
        env_vars: Vec<EnvVar>,
        env_comments: Vec<EnvComment>,
    },

    // Services
    ServiceCreate(ServiceSpec),
    ServiceUpdate {
        id: String,
        spec: ServiceSpec,
    },
    ServiceDelete {
        id: String,
    },
    ServiceList {
        project_id: String,
    },
    ServiceGet {
        id: String,
    },

    // Deployments
    DeployStart {
        service_id: String,
    },
    DeployAbort {
        deployment_id: String,
    },
    DeployRollback {
        service_id: String,
    },
    DeployHistory {
        service_id: String,
        limit: usize,
    },
    DeployDelete {
        deployment_id: String,
    },

    // Service lifecycle
    ServiceStop {
        service_id: String,
    },
    ServiceReload {
        service_id: String,
    },

    // Global views
    RecentDeployments {
        limit: usize,
    },
    GetBuildLogs {
        deployment_id: String,
    },

    // Observability
    LogsGet {
        service_id: String,
        tail: usize,
    },
    LogsSubscribe {
        service_id: String,
        tail: usize,
    },
    LogsUnsubscribe {
        service_id: String,
    },
    MetricsSubscribe {
        service_id: String,
    },
    MetricsUnsubscribe {
        service_id: String,
    },

    /// Host e URL de conexão **dentro** da rede Docker do projeto (montada no
    /// daemon por `shared::connection`, para GUI e webui não divergirem).
    ServiceConnectionInfo {
        service_id: String,
    },

    /// Projetos autorizados a alcançar um servidor de banco compartilhado.
    SharedAccessList {
        server_service_id: String,
    },
    /// Autoriza um projeto: conecta o servidor à rede dele sob o alias global.
    SharedAccessGrant {
        server_service_id: String,
        project_id: String,
    },
    SharedAccessRevoke {
        server_service_id: String,
        project_id: String,
    },

    /// Databases gerenciados de um servidor compartilhado.
    ManagedDatabaseList {
        server_service_id: String,
    },
    /// Cria database + usuário, autoriza o projeto e grava a env var nele.
    ManagedDatabaseCreate(crate::ManagedDatabaseCreateReq),
    /// Remove database + usuário do servidor (**apaga os dados**).
    ManagedDatabaseDelete {
        id: String,
    },

    /// Todos os databases gerenciados (de todos os servidores).
    ManagedDatabaseListAll,
    /// Inicia a migração (corre em segundo plano; acompanhe com `MigrationGet`).
    MigrationStart(crate::MigrationStartReq),
    MigrationGet {
        id: String,
    },
    MigrationList {
        project_id: String,
    },
    /// Volta a app para o banco antigo (env var + serviços).
    MigrationRollback {
        id: String,
    },
    /// Remove o banco antigo (**apaga os dados dele**) após o período de observação.
    MigrationDiscard {
        id: String,
    },

    // Webhooks
    GetWebhookUrl {
        service_id: String,
    },
    RegenerateWebhookToken {
        service_id: String,
    },
    GetDaemonSettings,
    SetDaemonSettings {
        acme_email: Option<String>,
        registry_domain: Option<String>,
    },

    // Secrets
    SecretSet {
        project_id: String,
        name: String,
        value: String,
    },
    SecretDelete {
        project_id: String,
        name: String,
    },
    SecretList {
        project_id: String,
    },

    // Infra-as-Code (manifesto declarativo)
    /// Reconcilia projetos/serviços a partir de manifestos YAML já interpolados
    /// pelo cliente (um documento `ProjectManifest` por string). Aditivo:
    /// cria/atualiza, nunca deleta. Não dispara deploy.
    ///
    /// Os manifestos trafegam como YAML (e não como structs) para que o daemon
    /// receba exatamente o texto do arquivo que o usuário edita; o parse fica
    /// num lugar só, com `serde_yaml`. Ver `docs/infra-as-code.md`.
    ManifestApply {
        manifests: Vec<String>,
        /// Deleta serviços que existem no projeto mas não constam no manifesto.
        prune: bool,
        /// Dispara deploy dos serviços criados/alterados após sincronizar.
        deploy: bool,
    },
    /// Exporta o estado atual de um projeto como manifesto YAML (secrets redigidos).
    ManifestExport {
        project_id: String,
    },
    /// Exporta TODOS os projetos+serviços num único manifesto raiz
    /// (`ServerManifest`). Todo valor de env var `Plain` é redigido para
    /// `${KEY}` (nunca o valor real no YAML); `Secret` continua como
    /// `secret:NOME`. Resposta: `ManifestBundle` (YAML + `.env` complementar
    /// com os valores reais das vars `Plain`).
    ManifestExportAll,
    /// Importa um manifesto raiz (`projects:`) ou de projeto único
    /// (`project:`) junto com o texto `.env` que resolve os `${VAR}` nele
    /// usados. A interpolação roda no daemon (reaproveita
    /// `ProjectManifest::interpolate`); se sobrar alguma `${VAR}` sem valor em
    /// qualquer projeto, nada é aplicado e a resposta é `MissingEnvVars`. Sem
    /// faltantes, reconcilia exatamente como `ManifestApply` (aditivo por
    /// padrão; `prune`/`deploy` com a mesma semântica).
    ManifestImport {
        yaml: String,
        dotenv: String,
        prune: bool,
        deploy: bool,
    },

    // Pacote de serviço (copiar um serviço entre servidores) — ver
    // `docs/plano-copiar-servico-entre-servidores.md`.
    /// O que dá para levar do serviço: suas variáveis e as do projeto (com a
    /// sugestão de pré-marcação). Nunca devolve valores. Resposta:
    /// `ServiceExportPlan`.
    ServiceExportPlan {
        service_id: String,
    },
    /// Gera o pacote (YAML) do serviço. `project_env_keys` são as variáveis do
    /// projeto marcadas pelo usuário (as demais não entram no arquivo);
    /// `include_values: false` leva só os nomes (`${CHAVE}`). Resposta:
    /// `ServiceBundleYaml`.
    ServiceExport {
        service_id: String,
        include_values: bool,
        #[serde(default)]
        project_env_keys: Vec<String>,
    },
    /// Cria UM serviço novo a partir de um pacote, sem sobrescrever nada.
    /// `dry_run` só devolve o relatório. Resposta: `ServiceImportReport`.
    ServiceImport(ServiceImportReq),

    // Jobs (tarefas one-shot via docker-compose, agendadas ou manuais)
    JobCreate {
        project_id: String,
        trigger_service_id: String,
        name: String,
        compose: String,
        /// `Some` = clona esse repo a cada execução em vez de usar `compose`
        /// (cliente manda `compose: ""` nesse caso). `#[serde(default)]`: os
        /// clientes só incluem esta chave no modo Git (Luau/JS table
        /// constructor com valor Lua/JS `nil`/`undefined` não gera a chave).
        #[serde(default)]
        git_source: Option<JobGitSource>,
        main_service: String,
        #[serde(default)]
        env_vars: Vec<EnvVar>,
        #[serde(default)]
        env_comments: Vec<EnvComment>,
        recurrence: Option<Recurrence>,
    },
    JobUpdate {
        id: String,
        name: String,
        compose: String,
        #[serde(default)]
        git_source: Option<JobGitSource>,
        main_service: String,
        #[serde(default)]
        env_vars: Vec<EnvVar>,
        #[serde(default)]
        env_comments: Vec<EnvComment>,
        enabled: bool,
        recurrence: Option<Recurrence>,
    },
    JobDelete {
        id: String,
    },
    /// Jobs de um único projeto (aba "Jobs" do projeto).
    JobList {
        project_id: String,
    },
    /// Todos os jobs, de todos os projetos (tela global da sidebar "Schedules").
    JobListAll,
    /// Dispara o job imediatamente, fora do agendamento.
    JobRunNow {
        id: String,
    },
    /// Cancela um `job_run` em andamento: mata o processo `docker compose
    /// up` de verdade (não só a task) — ver `AppState::active_jobs`. Erro
    /// `NotFound` se o run já não estiver mais registrado como em execução
    /// (terminou, ou o id nunca existiu).
    JobRunCancel {
        job_run_id: String,
    },
    JobRunHistory {
        job_id: String,
        limit: usize,
    },
    GetJobLogs {
        job_run_id: String,
    },
    /// Fonte única da verdade de "este job está quente?": confere se as imagens
    /// e o cache de build dele ainda estão no host. Resposta: `JobWarmStatus`.
    JobWarmStatus {
        job_id: String,
    },
    /// Aquece o job (baixa as imagens que faltam e popula o cache de build) sem
    /// executá-lo. Já quente: não faz nada e avisa. Senão a vaga única da fila
    /// é disputada em segundo plano e o fim chega em `Event::JobWarmChanged`.
    ///
    /// `force = true` **reconstrói do zero** mesmo com o job quente: rebaixa as
    /// imagens de registry e roda o build sem cache (`--no-cache --pull`), para
    /// pegar imagem base nova ou refazer um `RUN` que buscou algo externo.
    /// Ausente no JSON (clientes antigos) vale `false`.
    /// Resposta: `JobWarmRequested`.
    JobWarm {
        job_id: String,
        #[serde(default)]
        force: bool,
    },

    /// O status de aquecimento (`ServiceWarmStatus`) de todos os serviços com
    /// fonte Git ou zip. Resposta: `ServiceWarmStatuses`.
    ServiceWarmStatusList,
    /// Aquece o serviço: constrói a imagem dele na vaga única da fila, para o
    /// próximo deploy achar o cache quente. Já quente: não faz nada e avisa
    /// (a menos que `force`, que reconstrói sem cache). O fim chega em
    /// `Event::ServiceWarmChanged`. Resposta: `ServiceWarmRequested`.
    ServiceWarm {
        service_id: String,
        #[serde(default)]
        force: bool,
    },

    // Docker cleanup
    PruneContainers,
    /// `all=true` remove volumes mesmo que não sejam anônimos (equivalente ao
    /// `docker volume prune --all`); `false` é o padrão do Docker.
    PruneVolumes {
        all: bool,
    },
    /// `all=true` remove toda imagem sem uso, não só as dangling/untagged
    /// (equivalente ao `docker image prune -a`); `false` é o padrão do Docker.
    PruneImages {
        all: bool,
    },
    PruneBuildCache,
    PruneNetworks,

    // Limpeza automática (agendada) dos recursos acima — ver
    // docs/plano-limpeza-automatica-docker.md. `RunNow` dispara o mesmo
    // caminho de código do scheduler, fora do horário agendado (botão
    // "Executar agora" da tela).
    DockerCleanupConfigGet,
    DockerCleanupConfigSet {
        config: DockerCleanupConfig,
    },
    DockerCleanupRunNow,

    // Docker inventory (every image/volume/network on the host, not just
    // rustploy-managed ones — see `shared::DockerImageInfo` etc.)
    DockerImages,
    DockerVolumes,
    DockerNetworks,
    /// Todo container do host (rodando + parado), para a sub-aba Containers.
    DockerContainers,

    // Remoção INDIVIDUAL de um recurso Docker (o par por-item dos `Prune*`).
    // O Docker recusa remover recursos em uso (sem force) — o erro é propagado.
    RemoveContainer {
        id: String,
    },
    RemoveImage {
        id: String,
    },
    RemoveVolume {
        name: String,
    },
    RemoveNetwork {
        id: String,
    },
    /// Stops every container labeled `rustploy.managed=true`, regardless of
    /// what the DB's service status currently says (more robust than
    /// looping over `Service` rows one `ServiceStop` at a time — see
    /// `Command::ServiceStop`). Scoped to rustploy's own containers; never
    /// touches unrelated containers on the same Docker host.
    StopAllManaged,

    // Ingress proxy — a tabela de rotas viva do proxy reverso (em memória, não
    // no banco). É ela que decide para onde uma requisição HTTP vai agora, e
    // até aqui não havia como olhá-la nem corrigi-la sem um redeploy.
    /// Lê a tabela de rotas do ingress (domínios + portas). Diagnóstico de
    /// 502: a rota existe? para qual `ip:porta` ela aponta?
    IngressRoutes,
    /// Recalcula as rotas a partir dos containers que realmente existem no
    /// Docker, sem redeployar. `service_id = None` varre todos os serviços.
    /// É o mesmo caminho do reconcile de boot, exposto sob demanda.
    IngressReconcile {
        #[serde(default)]
        service_id: Option<String>,
    },

    // Env var backup / restore
    /// Lista os snapshots disponíveis (retorna Vec<String> com nomes de ficheiro).
    EnvBackupList,
    /// Restaura o snapshot com o nome dado (caminho relativo ao backup_dir).
    EnvBackupRestore {
        snapshot: String,
    },

    // Infrastructure
    Ping,
    DaemonStatus,
    DeployEngineStatus,
    /// Pergunta ao GitHub se há release mais nova que o daemon. Só leitura: não instala nada.
    /// O resultado fica em cache no daemon (o GitHub limita a API anônima a 60 req/h por IP);
    /// `force` ignora o cache, mas o daemon ainda espaça consultas forçadas em ~1 min.
    /// Resposta: `UpdateInfo` (ou `Err` com `UpdateCheckFailed` se o GitHub não responder).
    CheckUpdate {
        #[serde(default)]
        force: bool,
    },

    // Git providers (Gitea OAuth2 / PAT)
    GitProviderList,
    GitProviderCreate {
        kind: GitProviderKind,
        name: String,
        base_url: String,
        auth_mode: GitAuthMode,
        oauth_client_id: Option<String>,
        oauth_client_secret: Option<String>,
        /// Personal Access Token, when `auth_mode == Pat`.
        pat: Option<String>,
    },
    GitProviderDelete {
        id: String,
    },
    /// Returns the Gitea authorization URL for the client to open in a browser.
    GitOAuthStart {
        provider_id: String,
    },
    GitRepoList {
        provider_id: String,
    },
    GitBranchList {
        provider_id: String,
        repo_full_name: String,
    },

    // Wizard "Novo serviço" (catálogos + criação server-side). O cliente Luau
    // só dirige a UI; o daemon monta o ServiceSpec via `shared::wizard` (que tem
    // acesso aos blueprints de `templates`).
    WizardCatalog {
        search: String,
    },
    WizardCreate(WizardCreateReq),
    /// Snapshot completo do dashboard como JSON (o mesmo que o SSE `/api/events`
    /// empurra a cada 2s). O cliente usa após uma mutação para refletir a
    /// mudança na hora, sem esperar o próximo tick. Resposta: `Snapshot(String)`.
    Snapshot,

    // Registry OCI embutido — sub-aba Docker > Registry (somente leitura +
    // delete; criar conteúdo só acontece via `docker push` externo).
    RegistryStatus,
    RegistryRepoList,
    RegistryTagList {
        repo: String,
    },
    /// Remove a tag; se outra tag apontar pro mesmo digest (mesmo manifest),
    /// ela também é removida — mesma semântica do DELETE da OCI spec (por
    /// digest, não por tag).
    RegistryTagDelete {
        repo: String,
        tag: String,
    },
    RegistryRepoDelete {
        repo: String,
    },
    /// Garbage collection do registry: remove manifests pendurados (sem tag e
    /// não referenciados por nenhum index), blobs órfãos (metadados + arquivos
    /// do CAS) e uploads abandonados há mais de 24 h. Resposta:
    /// `RegistryGcResult`.
    RegistryGc,
    /// Cria um token de acesso (Basic auth) — `scope` é `"pull"` ou `"push"`.
    /// Resposta traz o segredo em texto plano UMA ÚNICA VEZ.
    RegistryTokenCreate {
        name: String,
        scope: String,
    },
    RegistryTokenList,
    RegistryTokenRevoke {
        name: String,
    },
    /// Move um deploy enfileirado para o início da fila ("furar fila"). Sem
    /// efeito se o id não estiver na fila (ex.: já rodando/terminado).
    /// Resposta: `Ok`.
    DeployQueuePromote {
        deployment_id: String,
    },
    /// Reordena a fila para exatamente a ordem dada (ids de deployment). Ids
    /// ausentes/desconhecidos são ignorados; enfileirados omitidos ficam ao fim
    /// preservando a ordem relativa. Usado pelo drag-and-drop da GUI.
    /// Resposta: `Ok`.
    DeployQueueReorder {
        order: Vec<String>,
    },
    /// Pausa (`true`) ou retoma (`false`) a fila global. Pausada, o worker não
    /// puxa o próximo deploy; o que já estiver rodando segue até o fim.
    /// Resposta: `Ok`.
    DeployQueuePause {
        paused: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::Command;

    #[test]
    fn job_warm_sem_force_vale_false() {
        // Cliente antigo (GUI/webui/agente) não manda `force`.
        let cmd: Command = serde_json::from_str(r#"{"JobWarm":{"job_id":"job_1"}}"#).unwrap();
        assert!(matches!(cmd, Command::JobWarm { force: false, .. }));
    }

    #[test]
    fn check_update_sem_force_vale_false() {
        let cmd: Command = serde_json::from_str(r#"{"CheckUpdate":{}}"#).unwrap();
        assert!(matches!(cmd, Command::CheckUpdate { force: false }));
    }

    #[test]
    fn job_warm_com_force() {
        let cmd: Command =
            serde_json::from_str(r#"{"JobWarm":{"job_id":"job_1","force":true}}"#).unwrap();
        assert!(matches!(cmd, Command::JobWarm { force: true, .. }));
    }
}
