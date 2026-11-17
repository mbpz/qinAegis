//! Versioned model profiles and immutable, explicitly approved routing snapshots.
use crate::asset_file::{AssetFile, AssetGuard};
use crate::llm::{ArcLlmClient, ChatOptions, LlmClient, LlmError, Message};
use anyhow::{ensure, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

mod anthropic;

const MAX_REGISTRY_BYTES: usize = 1024 * 1024;
const MAX_CANDIDATES: usize = 4;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelProfile {
    pub id: String,
    pub label: String,
    pub adapter: String,
    pub base_url: String,
    pub model: String,
    #[serde(default, skip_serializing)]
    pub api_key: String,
    pub capabilities: Vec<String>,
    pub enabled: bool,
    /// Explicit consent to send task data to a non-loopback destination.
    pub allow_remote: bool,
    pub midscene_family: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskModelRoute {
    pub primary: String,
    #[serde(default)]
    pub fallbacks: Vec<String>,
}

#[derive(Serialize)]
pub struct ModelProfileSnapshot {
    #[serde(flatten)]
    pub profile: ModelProfile,
    pub api_key: String,
    pub api_key_configured: bool,
}

#[derive(Serialize)]
pub struct ModelRegistrySnapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub profiles: Vec<ModelProfileSnapshot>,
    pub routes: BTreeMap<String, TaskModelRoute>,
    pub project_routes: BTreeMap<String, BTreeMap<String, TaskModelRoute>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: u32,
    revision: u64,
    profiles: Vec<ModelProfile>,
    routes: BTreeMap<String, TaskModelRoute>,
    #[serde(default)]
    project_routes: BTreeMap<String, BTreeMap<String, TaskModelRoute>>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            profiles: Vec::new(),
            routes: BTreeMap::new(),
            project_routes: BTreeMap::new(),
        }
    }
}

impl Document {
    fn snapshot(&self) -> ModelRegistrySnapshot {
        ModelRegistrySnapshot {
            schema_version: 1,
            revision: self.revision,
            profiles: self
                .profiles
                .iter()
                .map(|p| {
                    let mut profile = p.clone();
                    let configured = !profile.api_key.is_empty();
                    profile.api_key.clear();
                    ModelProfileSnapshot {
                        profile,
                        api_key: String::new(),
                        api_key_configured: configured,
                    }
                })
                .collect(),
            routes: self.routes.clone(),
            project_routes: self.project_routes.clone(),
        }
    }

    fn save(&mut self, guard: &AssetGuard<'_>) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .context("model registry revision exhausted")?;
        // Secrets are serialized only at this private persistence boundary.
        let profiles: Vec<_> = self
            .profiles
            .iter()
            .map(|profile| {
                let mut value = serde_json::to_value(profile).expect("model profile serialization");
                value["api_key"] = serde_json::Value::String(profile.api_key.clone());
                value
            })
            .collect();
        let bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version":1,"revision":self.revision,"profiles":profiles,"routes":self.routes,"project_routes":self.project_routes
        }))?;
        guard.write(&bytes)
    }
}

/// A protocol adapter boundary, separate from tasks and provider labels.
pub trait ModelAdapterFactory: Send + Sync {
    fn adapter(&self) -> &str;
    fn build(&self, profile: &ModelProfile) -> Result<ArcLlmClient>;
}

pub struct ModelAdapters {
    factories: BTreeMap<String, Arc<dyn ModelAdapterFactory>>,
}

impl Default for ModelAdapters {
    fn default() -> Self {
        let mut value = Self {
            factories: BTreeMap::new(),
        };
        value
            .register(Arc::new(OpenAiCompatibleFactory))
            .expect("builtin adapter");
        value
            .register(Arc::new(anthropic::AnthropicFactory))
            .expect("builtin adapter");
        value
    }
}

impl ModelAdapters {
    pub fn register(&mut self, factory: Arc<dyn ModelAdapterFactory>) -> Result<()> {
        ensure!(valid_id(factory.adapter()), "invalid adapter id");
        ensure!(
            !self.factories.contains_key(factory.adapter()),
            "adapter already registered"
        );
        self.factories.insert(factory.adapter().to_owned(), factory);
        Ok(())
    }
}

pub struct ModelRegistry {
    file: AssetFile,
    adapters: Arc<ModelAdapters>,
}

impl ModelRegistry {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self::with_adapters(path, Arc::new(ModelAdapters::default()))
    }

    pub fn with_adapters(path: impl Into<PathBuf>, adapters: Arc<ModelAdapters>) -> Self {
        Self {
            file: AssetFile::new(path.into(), MAX_REGISTRY_BYTES),
            adapters,
        }
    }

    fn read(&self, guard: &AssetGuard<'_>) -> Result<Document> {
        let doc: Document = match guard.read()? {
            Some(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| anyhow::anyhow!("invalid model registry document"))?,
            None => Document::default(),
        };
        ensure!(doc.schema_version == 1, "unsupported model registry schema");
        ensure!(doc.profiles.len() <= 128, "too many model profiles");
        let mut ids = BTreeSet::new();
        for profile in &doc.profiles {
            self.validate_profile(profile)?;
            ensure!(ids.insert(&profile.id), "duplicate model profile id");
        }
        for (task, route) in &doc.routes {
            validate_route(task, route, &doc.profiles)?;
        }
        ensure!(
            doc.project_routes.len() <= 256,
            "too many project model scopes"
        );
        for (project, routes) in &doc.project_routes {
            validate_project(project)?;
            for (task, route) in routes {
                validate_route(task, route, &doc.profiles)?;
            }
        }
        Ok(doc)
    }

    pub fn snapshot(&self) -> Result<ModelRegistrySnapshot> {
        let guard = self.file.lock()?;
        Ok(self.read(&guard)?.snapshot())
    }

    pub fn upsert(
        &self,
        expected_revision: u64,
        mut profile: ModelProfile,
        clear_api_key: bool,
    ) -> Result<ModelRegistrySnapshot> {
        let guard = self.file.lock()?;
        let mut doc = self.read(&guard)?;
        ensure!(
            doc.revision == expected_revision,
            "model registry revision conflict"
        );
        let existing = doc.profiles.iter().position(|p| p.id == profile.id);
        if clear_api_key {
            ensure!(
                profile.api_key.is_empty(),
                "cannot set and clear API key together"
            );
        } else if profile.api_key.is_empty() {
            if let Some(index) = existing {
                ensure!(
                    profile.base_url == doc.profiles[index].base_url
                        && profile.adapter == doc.profiles[index].adapter,
                    "changing endpoint or adapter requires an explicit API key"
                );
                profile.api_key = doc.profiles[index].api_key.clone();
            }
        }
        self.validate_profile(&profile)?;
        if let Some(index) = existing {
            doc.profiles[index] = profile;
        } else {
            ensure!(doc.profiles.len() < 128, "too many model profiles");
            doc.profiles.push(profile);
        }
        doc.save(&guard)?;
        Ok(doc.snapshot())
    }

    pub fn set_route(
        &self,
        expected_revision: u64,
        task: &str,
        route: TaskModelRoute,
    ) -> Result<ModelRegistrySnapshot> {
        let guard = self.file.lock()?;
        let mut doc = self.read(&guard)?;
        ensure!(
            doc.revision == expected_revision,
            "model registry revision conflict"
        );
        validate_route(task, &route, &doc.profiles)?;
        // Reject an unusable chain before persisting it.
        resolve_profiles(task, &route, &doc.profiles, &[])?;
        doc.routes.insert(task.to_owned(), route);
        doc.save(&guard)?;
        Ok(doc.snapshot())
    }

    /// None removes the override; subsequent runs use the global route.
    pub fn set_project_route(
        &self,
        expected_revision: u64,
        project: &str,
        task: &str,
        route: Option<TaskModelRoute>,
    ) -> Result<ModelRegistrySnapshot> {
        validate_project(project)?;
        validate_task(task)?;
        let guard = self.file.lock()?;
        let mut doc = self.read(&guard)?;
        ensure!(
            doc.revision == expected_revision,
            "model registry revision conflict"
        );
        if let Some(route) = route {
            resolve_profiles(task, &route, &doc.profiles, &[])?;
            ensure!(
                doc.project_routes.contains_key(project) || doc.project_routes.len() < 256,
                "too many project model scopes"
            );
            doc.project_routes
                .entry(project.to_owned())
                .or_default()
                .insert(task.to_owned(), route);
        } else if let Some(routes) = doc.project_routes.get_mut(project) {
            routes.remove(task);
            if routes.is_empty() {
                doc.project_routes.remove(project);
            }
        }
        doc.save(&guard)?;
        Ok(doc.snapshot())
    }

    pub fn resolve(&self, task: &str, required_caps: &[&str]) -> Result<ResolvedModelRoute> {
        self.resolve_for_project(None, task, required_caps)
    }

    pub fn resolve_for_project(
        &self,
        project: Option<&str>,
        task: &str,
        required_caps: &[&str],
    ) -> Result<ResolvedModelRoute> {
        validate_task(task)?;
        if let Some(project) = project {
            validate_project(project)?;
        }
        let guard = self.file.lock()?;
        let doc = self.read(&guard)?;
        let scoped = project
            .and_then(|p| doc.project_routes.get(p))
            .and_then(|routes| routes.get(task));
        let route = scoped
            .or_else(|| doc.routes.get(task))
            .context("no model route configured for task")?;
        let (profiles, capabilities) = resolve_profiles(task, route, &doc.profiles, required_caps)?;
        Ok(ResolvedModelRoute {
            revision: doc.revision,
            task: task.to_owned(),
            profiles,
            capabilities,
            adapters: self.adapters.clone(),
        })
    }

    /// Capture one approved destination, never a fallback chain. No network I/O under the file lock.
    pub fn prepare_probe(&self, profile_id: &str) -> Result<ModelProbe> {
        let guard = self.file.lock()?;
        let doc = self.read(&guard)?;
        let route = TaskModelRoute {
            primary: profile_id.to_owned(),
            fallbacks: vec![],
        };
        let (profiles, _) = resolve_profiles("generate", &route, &doc.profiles, &[])?;
        let profile = profiles
            .into_iter()
            .next()
            .context("model profile missing")?;
        let client = self
            .adapters
            .factories
            .get(&profile.adapter)
            .context("unknown model adapter")?
            .build(&profile)?;
        Ok(ModelProbe {
            revision: doc.revision,
            profile,
            client,
        })
    }

    fn validate_profile(&self, p: &ModelProfile) -> Result<()> {
        ensure!(valid_id(&p.id), "invalid model profile id");
        ensure!(
            !p.label.trim().is_empty() && p.label.len() <= 200,
            "invalid model label"
        );
        ensure!(
            !p.model.trim().is_empty() && p.model.len() <= 200,
            "invalid model name"
        );
        ensure!(
            self.adapters.factories.contains_key(&p.adapter),
            "unknown model adapter"
        );
        let url = reqwest::Url::parse(&p.base_url)
            .map_err(|_| anyhow::anyhow!("invalid model endpoint"))?;
        ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "invalid model endpoint"
        );
        ensure!(
            p.api_key.len() <= 8192
                && !p.api_key.contains(['\r', '\n'])
                && reqwest::header::HeaderValue::from_str(&format!("Bearer {}", p.api_key)).is_ok(),
            "invalid API key"
        );
        ensure!(!p.capabilities.is_empty(), "model capabilities required");
        let mut capabilities = BTreeSet::new();
        for capability in &p.capabilities {
            validate_capability(capability)?;
            ensure!(
                capabilities.insert(capability),
                "duplicate model capability"
            );
        }
        if p.adapter == "anthropic_messages" {
            ensure!(
                p.capabilities
                    .iter()
                    .all(|c| matches!(c.as_str(), "text" | "json"))
                    && p.midscene_family.is_none(),
                "Anthropic Messages adapter supports text/JSON only"
            );
        }
        if let Some(family) = &p.midscene_family {
            ensure!(
                !family.trim().is_empty()
                    && family.len() <= 100
                    && !family.chars().any(char::is_control),
                "invalid Midscene family"
            );
        }
        Ok(())
    }
}

fn validate_project(project: &str) -> Result<()> {
    ensure!(
        !project.is_empty()
            && project.len() <= 128
            && !project.starts_with('.')
            && project
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')),
        "invalid project model scope"
    );
    Ok(())
}

/// Synthetic observations, not provider declarations or visual-engine certification.
#[derive(Debug, Serialize)]
pub struct ModelProbeResult {
    pub revision: u64,
    pub profile_id: String,
    pub model: String,
    pub text: bool,
    pub json: Option<bool>,
    pub vision: Option<bool>,
    pub tools: Option<bool>,
    pub error: Option<String>,
}

pub struct ModelProbe {
    revision: u64,
    profile: ModelProfile,
    client: ArcLlmClient,
}
impl ModelProbe {
    pub async fn run(self) -> ModelProbeResult {
        self.run_with_timeout(Duration::from_secs(10)).await
    }
    async fn run_with_timeout(self, budget: Duration) -> ModelProbeResult {
        let mut result = ModelProbeResult {
            revision: self.revision,
            profile_id: self.profile.id,
            model: self.profile.model,
            text: false,
            json: None,
            vision: None,
            tools: None,
            error: None,
        };
        let observation = tokio::time::timeout(budget, async {
            let response = self.client.chat_with_options(&[Message::user("Reply with exactly QINAEGIS_OK. Synthetic connection check; no project data.")],
                ChatOptions::new().with_max_tokens(32)).await?;
            result.text = response.trim() == "QINAEGIS_OK";
            if self.profile.capabilities.iter().any(|c| c == "json") {
                let response = self.client.chat_with_options(&[Message::user("Reply with exactly {\"ok\":true}. Synthetic JSON check; no project data.")],
                    ChatOptions::new().with_max_tokens(32).with_json_schema(r#"{"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}"#)).await?;
                result.json = Some(serde_json::from_str::<serde_json::Value>(&response).ok() == Some(serde_json::json!({"ok":true})));
            }
            Ok::<(), LlmError>(())
        }).await;
        result.error = match observation {
            Ok(Ok(())) => None,
            Ok(Err(_)) => Some("model probe request failed".into()),
            Err(_) => Some("model probe timed out".into()),
        };
        result
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn validate_task(task: &str) -> Result<()> {
    ensure!(
        matches!(task, "generate" | "explore" | "execute" | "correct"),
        "unknown model task"
    );
    Ok(())
}
fn validate_capability(capability: &str) -> Result<()> {
    ensure!(
        matches!(capability, "text" | "vision" | "json" | "tools"),
        "unknown model capability"
    );
    Ok(())
}
fn validate_route(task: &str, route: &TaskModelRoute, profiles: &[ModelProfile]) -> Result<()> {
    validate_task(task)?;
    ensure!(
        !matches!(task, "explore" | "execute") || route.fallbacks.is_empty(),
        "visual executor routes do not support automatic fallback"
    );
    ensure!(
        route.fallbacks.len() < MAX_CANDIDATES,
        "too many model fallbacks"
    );
    let mut ids = BTreeSet::new();
    for id in std::iter::once(&route.primary).chain(&route.fallbacks) {
        ensure!(
            valid_id(id) && ids.insert(id),
            "invalid or duplicate route profile id"
        );
        ensure!(
            profiles.iter().any(|p| &p.id == id),
            "route profile not found"
        );
    }
    Ok(())
}
fn resolve_profiles(
    task: &str,
    route: &TaskModelRoute,
    profiles: &[ModelProfile],
    required: &[&str],
) -> Result<(Vec<ModelProfile>, BTreeSet<String>)> {
    validate_route(task, route, profiles)?;
    let mut capabilities = BTreeSet::from(["text".to_owned()]);
    for cap in required {
        validate_capability(cap)?;
        capabilities.insert((*cap).to_owned());
    }
    let visual = matches!(task, "explore" | "execute") || capabilities.contains("vision");
    if visual {
        capabilities.insert("vision".to_owned());
    }
    let mut selected = Vec::new();
    for id in std::iter::once(&route.primary).chain(&route.fallbacks) {
        let profile = profiles
            .iter()
            .find(|p| &p.id == id)
            .context("route profile not found")?;
        ensure!(
            profile.enabled && !profile.api_key.is_empty(),
            "route profile disabled or missing API key"
        );
        ensure!(
            capabilities
                .iter()
                .all(|c| profile.capabilities.contains(c)),
            "route profile missing required capability"
        );
        ensure!(
            !visual || profile.midscene_family.is_some(),
            "visual route needs Midscene family"
        );
        let endpoint = reqwest::Url::parse(&profile.base_url)
            .map_err(|_| anyhow::anyhow!("invalid model endpoint"))?;
        let local = matches!(
            endpoint.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        );
        ensure!(
            local || profile.allow_remote,
            "remote model destination needs explicit authorization"
        );
        selected.push(profile.clone());
    }
    Ok((selected, capabilities))
}

/// Holds private cloned credentials; never serialize or Debug this runtime snapshot.
pub struct ResolvedModelRoute {
    revision: u64,
    task: String,
    profiles: Vec<ModelProfile>,
    capabilities: BTreeSet<String>,
    adapters: Arc<ModelAdapters>,
}
impl ResolvedModelRoute {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn task(&self) -> &str {
        &self.task
    }
    pub fn candidates(&self) -> &[ModelProfile] {
        &self.profiles
    }
}

/// Bounded metadata only; callers must snapshot and atomically save it with their asset.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelCallAttempt {
    pub call: u64,
    pub attempt: u8,
    pub profile_id: String,
    pub outcome: ModelCallOutcome,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelCallOutcome {
    Succeeded,
    TransportFailure,
    TimedOut,
    Rejected,
}
#[derive(Default)]
pub struct ModelCallTrace {
    next_call: std::sync::atomic::AtomicU64,
    entries: std::sync::Mutex<(Vec<ModelCallAttempt>, bool)>,
}
impl ModelCallTrace {
    pub fn snapshot(&self) -> Result<Vec<ModelCallAttempt>> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| anyhow::anyhow!("model trace unavailable"))?;
        ensure!(!entries.1, "model trace capacity exceeded");
        Ok(entries.0.clone())
    }
    fn record(&self, entry: ModelCallAttempt) {
        if let Ok(mut entries) = self.entries.lock() {
            if entries.0.len() < 1024 {
                entries.0.push(entry);
            } else {
                entries.1 = true;
            }
        }
    }
}

pub fn build_client(route: &ResolvedModelRoute) -> Result<ArcLlmClient> {
    build_registered_client(route, None)
}
/// Credentials/inputs/outputs/errors never enter the trace. Call identity groups fallback attempts.
pub fn build_client_with_trace(
    route: &ResolvedModelRoute,
) -> Result<(ArcLlmClient, Arc<ModelCallTrace>)> {
    let trace = Arc::new(ModelCallTrace::default());
    let client = build_registered_client(route, Some(trace.clone()))?;
    Ok((client, trace))
}
fn build_registered_client(
    route: &ResolvedModelRoute,
    trace: Option<Arc<ModelCallTrace>>,
) -> Result<ArcLlmClient> {
    let mut clients = Vec::new();
    for profile in &route.profiles {
        let factory = route
            .adapters
            .factories
            .get(&profile.adapter)
            .context("unknown model adapter")?;
        clients.push((profile.id.clone(), factory.build(profile)?));
    }
    ensure!(!clients.is_empty(), "empty model route");
    Ok(ArcLlmClient::new(RegisteredClient {
        clients,
        capabilities: route.capabilities.clone(),
        trace,
    }))
}

struct RegisteredClient {
    clients: Vec<(String, ArcLlmClient)>,
    capabilities: BTreeSet<String>,
    trace: Option<Arc<ModelCallTrace>>,
}
#[async_trait]
impl LlmClient for RegisteredClient {
    async fn chat(&self, messages: &[Message]) -> Result<String, LlmError> {
        self.chat_with_options(messages, ChatOptions::default())
            .await
    }
    async fn chat_with_options(
        &self,
        messages: &[Message],
        options: ChatOptions,
    ) -> Result<String, LlmError> {
        if options.vision == Some(true) {
            return Err(LlmError::UnsupportedCapability(
                "chat image input requires a visual executor",
            ));
        }
        if options.json_schema.is_some() && !self.capabilities.contains("json") {
            return Err(LlmError::UnsupportedCapability("json"));
        }
        self.chat_with_budget(messages, options, Duration::from_secs(30))
            .await
    }
}

impl RegisteredClient {
    async fn chat_with_budget(
        &self,
        messages: &[Message],
        options: ChatOptions,
        total_budget: Duration,
    ) -> Result<String, LlmError> {
        let call = self
            .trace
            .as_ref()
            .map(|t| {
                t.next_call
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    + 1
            })
            .unwrap_or(0);
        let candidate_budget = total_budget / self.clients.len().max(1) as u32;
        let deadline = tokio::time::Instant::now() + total_budget;
        for (index, (id, client)) in self.clients.iter().enumerate() {
            if tokio::time::Instant::now() >= deadline {
                return Err(LlmError::ModelFailure("model route timed out"));
            }
            let response = tokio::time::timeout_at(
                deadline.min(tokio::time::Instant::now() + candidate_budget),
                client.chat_with_options(messages, options.clone()),
            )
            .await;
            if let Some(trace) = &self.trace {
                let outcome = match &response {
                    Ok(Ok(_)) => ModelCallOutcome::Succeeded,
                    Ok(Err(LlmError::ModelFailure(_))) | Ok(Err(LlmError::Request(_))) => {
                        ModelCallOutcome::TransportFailure
                    }
                    Err(_) => ModelCallOutcome::TimedOut,
                    Ok(Err(_)) => ModelCallOutcome::Rejected,
                };
                trace.record(ModelCallAttempt {
                    call,
                    attempt: (index + 1) as u8,
                    profile_id: id.clone(),
                    outcome,
                });
            }
            match response {
                Ok(Ok(result)) => {
                    tracing::info!(provider_id = %id, "model request completed");
                    return Ok(result);
                }
                Ok(Err(LlmError::ModelFailure(_))) | Ok(Err(LlmError::Request(_))) | Err(_) => {
                    tracing::warn!(
                        provider_id = %id,
                        fallback = index + 1 < self.clients.len(),
                        "model request failed"
                    );
                }
                Ok(Err(_)) => {
                    return Err(LlmError::Api(
                        "model request rejected without fallback".to_owned(),
                    ))
                }
            }
        }
        Err(LlmError::ModelFailure("approved model route exhausted"))
    }
}

struct OpenAiCompatibleFactory;
impl ModelAdapterFactory for OpenAiCompatibleFactory {
    fn adapter(&self) -> &str {
        "openai_compatible"
    }
    fn build(&self, profile: &ModelProfile) -> Result<ArcLlmClient> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| anyhow::anyhow!("cannot initialize model HTTP client"))?;
        Ok(ArcLlmClient::new(OpenAiCompatibleClient {
            profile: profile.clone(),
            client,
        }))
    }
}
struct OpenAiCompatibleClient {
    profile: ModelProfile,
    client: reqwest::Client,
}
#[derive(Deserialize)]
struct Completion {
    choices: Vec<CompletionChoice>,
}
#[derive(Deserialize)]
struct CompletionChoice {
    message: Message,
}
#[async_trait]
impl LlmClient for OpenAiCompatibleClient {
    async fn chat(&self, messages: &[Message]) -> Result<String, LlmError> {
        self.chat_with_options(messages, ChatOptions::default())
            .await
    }
    async fn chat_with_options(
        &self,
        messages: &[Message],
        options: ChatOptions,
    ) -> Result<String, LlmError> {
        let mut conversation = messages.to_vec();
        if let Some(system) = options.system_prompt {
            conversation.insert(0, Message::system(system));
        }
        let mut body = serde_json::json!({"model":self.profile.model,"messages":conversation,"max_tokens":options.max_tokens.unwrap_or(4096)});
        if let Some(temperature) = options.temperature {
            body["temperature"] = serde_json::json!(temperature);
        }
        if let Some(schema) = options.json_schema {
            let schema: serde_json::Value = serde_json::from_str(&schema)
                .map_err(|_| LlmError::UnsupportedCapability("invalid JSON schema"))?;
            body["response_format"] = serde_json::json!({"type":"json_schema","json_schema":{"name":"response","strict":true,"schema":schema}});
        }
        let response = self
            .client
            .post(format!(
                "{}/chat/completions",
                self.profile.base_url.trim_end_matches('/')
            ))
            .bearer_auth(&self.profile.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|_| LlmError::ModelFailure("model transport failure"))?;
        if !response.status().is_success() {
            return Err(LlmError::ModelFailure("model HTTP failure"));
        }
        // Bound response memory even when the upstream omits Content-Length.
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| LlmError::ModelFailure("model response transport failure"))?
        {
            if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
                return Err(LlmError::ModelFailure("model response exceeds limit"));
            }
            bytes.extend_from_slice(&chunk);
        }
        let completion: Completion = serde_json::from_slice(&bytes)
            .map_err(|_| LlmError::ModelFailure("invalid model response"))?;
        completion
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or(LlmError::ModelFailure("empty model response"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn profile(id: &str) -> ModelProfile {
        ModelProfile {
            id: id.to_owned(),
            label: id.to_owned(),
            adapter: "openai_compatible".to_owned(),
            base_url: "https://example.invalid/v1".to_owned(),
            model: format!("model-{id}"),
            api_key: "private-test-key".to_owned(),
            capabilities: vec!["text".to_owned()],
            enabled: true,
            allow_remote: true,
            midscene_family: None,
        }
    }
    fn route(primary: &str, fallbacks: &[&str]) -> TaskModelRoute {
        TaskModelRoute {
            primary: primary.to_owned(),
            fallbacks: fallbacks.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    #[test]
    fn registry_cas_and_secret_redaction_preserve_private_key() {
        let dir = tempfile::tempdir().unwrap();
        let registry = ModelRegistry::new(dir.path().join("models.json"));
        let snapshot = registry.upsert(0, profile("one"), false).unwrap();
        let serialized = serde_json::to_string(&snapshot).unwrap();
        assert!(!serialized.contains("private-test-key"));
        assert!(serialized.contains("\"api_key\":\"\""));
        assert!(snapshot.profiles[0].api_key_configured);
        assert!(registry.upsert(0, profile("two"), false).is_err());
        let mut edited = profile("one");
        edited.api_key.clear();
        registry.upsert(1, edited.clone(), false).unwrap();
        registry
            .set_route(2, "generate", route("one", &[]))
            .unwrap();
        assert_eq!(
            registry.resolve("generate", &[]).unwrap().candidates()[0].api_key,
            "private-test-key"
        );
        registry.upsert(3, edited, true).unwrap();
        assert!(!registry.snapshot().unwrap().profiles[0].api_key_configured);
        assert!(registry.resolve("generate", &[]).is_err());
    }

    #[test]
    fn model_switch_does_not_mutate_resolved_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let registry = ModelRegistry::new(dir.path().join("models.json"));
        registry.upsert(0, profile("one"), false).unwrap();
        registry.upsert(1, profile("two"), false).unwrap();
        registry
            .set_route(2, "generate", route("one", &[]))
            .unwrap();
        let old = registry.resolve("generate", &["text"]).unwrap();
        registry
            .set_route(3, "generate", route("two", &[]))
            .unwrap();
        let new = registry.resolve("generate", &[]).unwrap();
        assert_eq!(old.candidates()[0].model, "model-one");
        assert_eq!(old.revision(), 3);
        assert_eq!(new.candidates()[0].model, "model-two");
        assert_eq!(new.revision(), 4);
        assert!(!serde_json::to_string(&old.candidates()[0])
            .unwrap()
            .contains("private-test-key"));
    }

    #[test]
    fn visual_routes_and_remote_fallback_require_explicit_compatibility() {
        let dir = tempfile::tempdir().unwrap();
        let registry = ModelRegistry::new(dir.path().join("models.json"));
        registry.upsert(0, profile("text"), false).unwrap();
        assert!(registry
            .set_route(1, "execute", route("text", &[]))
            .is_err());
        let mut visual = profile("visual");
        visual.capabilities.push("vision".to_owned());
        registry.upsert(1, visual.clone(), false).unwrap();
        assert!(registry
            .set_route(2, "explore", route("visual", &[]))
            .is_err());
        visual.midscene_family = Some("qwen2.5-vl".to_owned());
        registry.upsert(2, visual, false).unwrap();
        registry
            .set_route(3, "execute", route("visual", &[]))
            .unwrap();
        assert!(registry.resolve("execute", &["json"]).is_err());
        assert!(registry
            .set_route(4, "execute", route("visual", &["text"]))
            .is_err());
        let mut denied = profile("denied");
        denied.allow_remote = false;
        registry.upsert(4, denied, false).unwrap();
        assert!(registry
            .set_route(5, "generate", route("text", &["denied"]))
            .is_err());
        assert!(registry
            .set_route(5, "generate", route("denied", &[]))
            .is_err());
        let mut fallback = profile("fallback");
        fallback.allow_remote = true;
        registry.upsert(5, fallback, false).unwrap();
        registry
            .set_route(6, "generate", route("text", &["fallback"]))
            .unwrap();
        assert_eq!(
            registry
                .resolve("generate", &[])
                .unwrap()
                .candidates()
                .len(),
            2
        );
    }

    #[test]
    fn loopback_destinations_do_not_require_remote_consent() {
        for endpoint in [
            "http://localhost:8000/v1",
            "http://127.0.0.1:8000/v1",
            "http://[::1]:8000/v1",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let registry = ModelRegistry::new(dir.path().join("models.json"));
            let mut local = profile("local");
            local.base_url = endpoint.to_owned();
            local.allow_remote = false;
            registry.upsert(0, local, false).unwrap();
            registry
                .set_route(1, "generate", route("local", &[]))
                .unwrap();
            assert_eq!(
                registry.resolve("generate", &[]).unwrap().candidates()[0].id,
                "local"
            );
        }
    }

    #[test]
    fn invalid_profiles_and_unknown_adapters_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let registry = ModelRegistry::new(dir.path().join("models.json"));
        for endpoint in [
            "https://user:pass@example.invalid/v1",
            "https://example.invalid/v1?token=secret",
            "https://example.invalid/#secret",
            "file:///tmp/model",
        ] {
            let mut p = profile("one");
            p.base_url = endpoint.to_owned();
            assert!(registry.upsert(0, p, false).is_err());
        }
        let mut p = profile("../escape");
        assert!(registry.upsert(0, p.clone(), false).is_err());
        p.id = "one".to_owned();
        p.adapter = "arbitrary_protocol".to_owned();
        assert!(registry.upsert(0, p.clone(), false).is_err());
        p.adapter = "openai_compatible".to_owned();
        p.capabilities = vec!["telepathy".to_owned()];
        assert!(registry.upsert(0, p, false).is_err());
    }

    #[test]
    fn invalid_header_keys_are_rejected_before_save_and_on_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("models.json");
        let registry = ModelRegistry::new(path.clone());
        registry.upsert(0, profile("one"), false).unwrap();
        let before = std::fs::read(&path).unwrap();
        for control in ['\0', '\u{7f}', '\r', '\n'] {
            let mut invalid = profile("one");
            invalid.api_key = format!("private-test-key{control}");
            let error = registry
                .upsert(1, invalid, false)
                .err()
                .unwrap()
                .to_string();
            assert!(!error.contains("private-test-key"));
            assert_eq!(std::fs::read(&path).unwrap(), before);
            assert_eq!(registry.snapshot().unwrap().revision, 1);
        }
        let mut corrupted: serde_json::Value = serde_json::from_slice(&before).unwrap();
        corrupted["profiles"][0]["api_key"] = serde_json::json!("private-test-key\u{0000}");
        let corrupted = serde_json::to_vec(&corrupted).unwrap();
        std::fs::write(&path, &corrupted).unwrap();
        let error = registry.snapshot().err().unwrap().to_string();
        assert!(!error.contains("private-test-key"));
        assert!(registry.upsert(1, profile("one"), false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), corrupted);
    }

    #[test]
    fn project_overrides_are_isolated_cas_checked_and_frozen() {
        let dir = tempfile::tempdir().unwrap();
        let registry = ModelRegistry::new(dir.path().join("models.json"));
        registry.upsert(0, profile("one"), false).unwrap();
        registry.upsert(1, profile("two"), false).unwrap();
        registry
            .set_route(2, "generate", route("one", &[]))
            .unwrap();
        registry
            .set_project_route(3, "project.a", "generate", Some(route("two", &[])))
            .unwrap();
        let frozen = registry
            .resolve_for_project(Some("project.a"), "generate", &[])
            .unwrap();
        assert_eq!(frozen.candidates()[0].id, "two");
        assert_eq!(
            registry
                .resolve_for_project(Some("other"), "generate", &[])
                .unwrap()
                .candidates()[0]
                .id,
            "one"
        );
        assert!(registry
            .set_project_route(3, "project.a", "generate", None)
            .is_err());
        assert!(registry
            .set_project_route(4, "../escape", "generate", None)
            .is_err());
        assert!(registry
            .set_project_route(4, "project.a", "execute", Some(route("one", &[])))
            .is_err());
        registry
            .set_project_route(4, "project.a", "generate", None)
            .unwrap();
        assert_eq!(
            registry
                .resolve_for_project(Some("project.a"), "generate", &[])
                .unwrap()
                .candidates()[0]
                .id,
            "one"
        );
        assert_eq!(frozen.candidates()[0].id, "two");
        let snapshot = registry.snapshot().unwrap();
        assert!(snapshot.project_routes.is_empty());
        assert!(!serde_json::to_string(&snapshot)
            .unwrap()
            .contains("private-test-key"));
    }

    #[tokio::test]
    async fn probe_calls_local_chat_protocol_with_synthetic_inputs_and_json_schema() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for expected in ["QINAEGIS_OK", r#"{"ok":true}"#] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut data = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let n = stream.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    data.extend_from_slice(&buffer[..n]);
                    if let Some(split) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&data[..split]).to_lowercase();
                        let length: usize = headers
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .unwrap()
                            .trim()
                            .parse()
                            .unwrap();
                        if data.len() >= split + 4 + length {
                            assert!(headers.starts_with("post /v1/chat/completions "));
                            let body: serde_json::Value =
                                serde_json::from_slice(&data[split + 4..]).unwrap();
                            assert_eq!(body["max_tokens"], 32);
                            assert!(body["messages"][0]["content"]
                                .as_str()
                                .unwrap()
                                .contains("Synthetic"));
                            if expected.starts_with('{') {
                                assert_eq!(body["response_format"]["type"], "json_schema");
                            }
                            break;
                        }
                    }
                }
                let response = serde_json::json!({"choices":[{"message":{"role":"assistant","content":expected}}]}).to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
            }
        });
        let dir = tempfile::tempdir().unwrap();
        let registry = ModelRegistry::new(dir.path().join("models.json"));
        let mut p = profile("local");
        p.base_url = format!("http://{address}/v1");
        p.allow_remote = false;
        p.capabilities.push("json".into());
        registry.upsert(0, p, false).unwrap();
        let result = registry.prepare_probe("local").unwrap().run().await;
        server.join().unwrap();
        assert!(result.text);
        assert_eq!(result.json, Some(true));
        assert!(result.error.is_none());
        assert_eq!(result.vision, None);
        assert_eq!(registry.snapshot().unwrap().revision, 1);
    }

    struct ProbeClient {
        response: &'static str,
    }
    #[async_trait]
    impl LlmClient for ProbeClient {
        async fn chat(&self, _: &[Message]) -> Result<String, LlmError> {
            Ok(self.response.into())
        }
    }
    fn probe(client: ArcLlmClient) -> ModelProbe {
        let mut p = profile("test");
        p.capabilities.push("json".into());
        ModelProbe {
            revision: 9,
            profile: p,
            client,
        }
    }
    #[tokio::test]
    async fn probe_observations_never_infer_vision_or_echo_provider_errors() {
        let result = probe(ArcLlmClient::new(ProbeClient {
            response: "QINAEGIS_OK",
        }))
        .run()
        .await;
        assert!(result.text);
        assert_eq!(result.json, Some(false));
        assert_eq!(result.vision, None);
        assert_eq!(result.tools, None);
        assert_eq!(result.revision, 9);
        let result = probe(ArcLlmClient::new(MockClient {
            id: "primary".into(),
            calls: Arc::new(AtomicUsize::new(0)),
            business_failure: true,
        }))
        .run()
        .await;
        assert_eq!(result.error.as_deref(), Some("model probe request failed"));
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("private business assertion"));
        let result = probe(ArcLlmClient::new(HangingClient {
            calls: Arc::new(AtomicUsize::new(0)),
        }))
        .run_with_timeout(Duration::from_millis(10))
        .await;
        assert_eq!(result.error.as_deref(), Some("model probe timed out"));
    }
    #[test]
    fn prepare_probe_enforces_remote_consent_without_mutating_registry() {
        let dir = tempfile::tempdir().unwrap();
        let registry = ModelRegistry::new(dir.path().join("models.json"));
        let mut denied = profile("denied");
        denied.allow_remote = false;
        registry.upsert(0, denied, false).unwrap();
        assert!(registry.prepare_probe("denied").is_err());
        assert!(registry.prepare_probe("missing").is_err());
        assert_eq!(registry.snapshot().unwrap().revision, 1);
    }

    struct MockFactory {
        calls: Arc<AtomicUsize>,
        business_failure: bool,
    }
    impl ModelAdapterFactory for MockFactory {
        fn adapter(&self) -> &str {
            "mock"
        }
        fn build(&self, profile: &ModelProfile) -> Result<ArcLlmClient> {
            Ok(ArcLlmClient::new(MockClient {
                id: profile.id.clone(),
                calls: self.calls.clone(),
                business_failure: self.business_failure,
            }))
        }
    }
    struct MockClient {
        id: String,
        calls: Arc<AtomicUsize>,
        business_failure: bool,
    }
    #[async_trait]
    impl LlmClient for MockClient {
        async fn chat(&self, _messages: &[Message]) -> Result<String, LlmError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.id == "primary" {
                if self.business_failure {
                    Err(LlmError::Api("private business assertion".to_owned()))
                } else {
                    Err(LlmError::ModelFailure("mock transport failure"))
                }
            } else {
                Ok(self.id.clone())
            }
        }
    }
    fn mock_route(
        business_failure: bool,
    ) -> (tempfile::TempDir, ResolvedModelRoute, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut adapters = ModelAdapters::default();
        adapters
            .register(Arc::new(MockFactory {
                calls: calls.clone(),
                business_failure,
            }))
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let registry =
            ModelRegistry::with_adapters(dir.path().join("models.json"), Arc::new(adapters));
        let mut primary = profile("primary");
        primary.adapter = "mock".to_owned();
        let mut fallback = profile("fallback");
        fallback.adapter = "mock".to_owned();
        fallback.allow_remote = true;
        registry.upsert(0, primary, false).unwrap();
        registry.upsert(1, fallback, false).unwrap();
        registry
            .set_route(2, "generate", route("primary", &["fallback"]))
            .unwrap();
        (dir, registry.resolve("generate", &[]).unwrap(), calls)
    }
    #[tokio::test]
    async fn only_model_failures_trigger_approved_fallback() {
        let (_dir, route, calls) = mock_route(false);
        let client = build_client(&route).unwrap();
        assert_eq!(
            client.chat(&[Message::user("test")]).await.unwrap(),
            "fallback"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let (_dir, route, calls) = mock_route(true);
        let error = build_client(&route)
            .unwrap()
            .chat(&[])
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(!error.contains("private business assertion"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn trace_groups_fallback_and_excludes_provider_details() {
        let (_dir, route, _) = mock_route(false);
        let (client, trace) = build_client_with_trace(&route).unwrap();
        client
            .chat(&[Message::user("private input")])
            .await
            .unwrap();
        let frozen = trace.snapshot().unwrap();
        assert_eq!(frozen.len(), 2);
        assert_eq!(frozen[0].call, frozen[1].call);
        assert_eq!(frozen[0].outcome, ModelCallOutcome::TransportFailure);
        assert_eq!(frozen[1].outcome, ModelCallOutcome::Succeeded);
        assert_eq!(frozen[1].profile_id, "fallback");
        client.chat(&[]).await.unwrap();
        assert_eq!(frozen.len(), 2);
        assert_eq!(trace.snapshot().unwrap().len(), 4);
        let serialized = serde_json::to_string(&frozen).unwrap();
        assert!(!serialized.contains("private"));
        let (_dir, route, _) = mock_route(true);
        let (client, trace) = build_client_with_trace(&route).unwrap();
        assert!(client.chat(&[]).await.is_err());
        assert_eq!(
            trace.snapshot().unwrap()[0].outcome,
            ModelCallOutcome::Rejected
        );
        let trace = ModelCallTrace::default();
        for _ in 0..1025 {
            trace.record(frozen[0].clone());
        }
        assert!(trace.snapshot().is_err());
    }

    #[tokio::test]
    async fn unsupported_call_capability_never_reaches_any_provider() {
        let (_dir, route, calls) = mock_route(false);
        let error = build_client(&route)
            .unwrap()
            .chat_with_options(&[], ChatOptions::new().with_vision())
            .await;
        assert!(matches!(error, Err(LlmError::UnsupportedCapability(_))));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    struct HangingClient {
        calls: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl LlmClient for HangingClient {
        async fn chat(&self, _messages: &[Message]) -> Result<String, LlmError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::future::pending().await
        }
    }

    #[tokio::test]
    async fn total_deadline_records_the_last_inflight_attempt() {
        let trace = Arc::new(ModelCallTrace::default());
        let client = RegisteredClient {
            clients: vec![(
                "last".into(),
                ArcLlmClient::new(HangingClient {
                    calls: Arc::new(AtomicUsize::new(0)),
                }),
            )],
            capabilities: BTreeSet::from(["text".into()]),
            trace: Some(trace.clone()),
        };
        assert!(client
            .chat_with_budget(&[], ChatOptions::default(), Duration::from_millis(10))
            .await
            .is_err());
        let saved = trace.snapshot().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].outcome, ModelCallOutcome::TimedOut);
    }

    #[tokio::test]
    async fn timed_out_primary_leaves_budget_for_fallback() {
        let calls = Arc::new(AtomicUsize::new(0));
        let client = RegisteredClient {
            clients: vec![
                (
                    "primary".into(),
                    ArcLlmClient::new(HangingClient {
                        calls: calls.clone(),
                    }),
                ),
                (
                    "fallback".into(),
                    ArcLlmClient::new(MockClient {
                        id: "fallback".into(),
                        calls: calls.clone(),
                        business_failure: false,
                    }),
                ),
            ],
            capabilities: BTreeSet::from(["text".into()]),
            trace: None,
        };
        let result = client
            .chat_with_budget(&[], ChatOptions::default(), Duration::from_millis(100))
            .await;
        assert_eq!(result.unwrap(), "fallback");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
