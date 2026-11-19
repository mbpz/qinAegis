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

    /// Freeze one explicitly selected visual profile. This authorizes no
    /// fallback and performs no network calls. Credentials stay in memory.
    pub fn resolve_execution_profile(
        &self,
        profile_id: &str,
        expected_revision: u64,
    ) -> Result<ResolvedModelRoute> {
        let guard = self.file.lock()?;
        let doc = self.read(&guard)?;
        ensure!(
            doc.revision == expected_revision,
            "model registry revision conflict"
        );
        let route = TaskModelRoute {
            primary: profile_id.into(),
            fallbacks: vec![],
        };
        let (profiles, capabilities) =
            resolve_profiles("execute", &route, &doc.profiles, &["text", "vision"])?;
        Ok(ResolvedModelRoute {
            revision: doc.revision,
            task: "execute".into(),
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

/// Private, generation-only compatibility route. Configured destinations retain
/// their existing authorization; provider labels never select a different wire.
/// Freeze resolved configuration before queueing. Never serialize or Debug it.
pub struct LegacyGenerationRoute {
    profiles: Vec<ModelProfile>,
}
impl LegacyGenerationRoute {
    pub fn freeze(
        primary: crate::llm::ProviderConfig,
        secondary: Option<crate::llm::ProviderConfig>,
    ) -> Result<Self> {
        let mut profiles = Vec::new();
        // Profile validation performs no file access.
        let validator = ModelRegistry::new(PathBuf::new());
        for (id, configured) in std::iter::once(("legacy-primary", primary))
            .chain(secondary.map(|p| ("legacy-secondary", p)))
        {
            ensure!(
                configured.is_configured(),
                "legacy generation model is incomplete"
            );
            let profile = ModelProfile {
                id: id.into(),
                label: id.into(),
                adapter: "openai_compatible".into(),
                base_url: configured.base_url,
                api_key: configured.api_key,
                model: configured.model,
                capabilities: vec!["text".into()],
                enabled: true,
                allow_remote: true,
                midscene_family: None,
            };
            validator.validate_profile(&profile)?;
            profiles.push(profile);
        }
        let route = Self { profiles };
        let metadata = route.snapshot().to_string();
        ensure!(
            route.profiles.iter().all(|p| !metadata.contains(&p.api_key)
                && route
                    .profiles
                    .iter()
                    .all(|candidate| !candidate.model.contains(&p.api_key))),
            "legacy generation metadata contains a credential"
        );
        Ok(route)
    }
    pub fn candidates(&self) -> &[ModelProfile] {
        &self.profiles
    }
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({"source":"legacy_config","model":self.profiles[0].model,
            "candidates":self.profiles.iter().map(|p| serde_json::json!({
                "id":p.id,"adapter":p.adapter,"model":p.model,
                "capabilities":p.capabilities,"midscene_family":p.midscene_family
            })).collect::<Vec<_>>()})
    }
    pub fn build_client_with_trace(&self) -> Result<(ArcLlmClient, Arc<ModelCallTrace>)> {
        let trace = Arc::new(ModelCallTrace::default());
        let mut clients = Vec::new();
        for profile in &self.profiles {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| anyhow::anyhow!("cannot initialize model HTTP client"))?;
            clients.push((
                profile.id.clone(),
                ArcLlmClient::new(OpenAiCompatibleClient {
                    profile: profile.clone(),
                    client,
                    default_max_tokens: 1024,
                }),
            ));
        }
        Ok((
            ArcLlmClient::new(RegisteredClient {
                clients,
                capabilities: BTreeSet::from(["text".into()]),
                trace: Some(trace.clone()),
            }),
            trace,
        ))
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
    /// Locally admitted; does not prove that the remote provider received it.
    Started,
    Succeeded,
    TransportFailure,
    TimedOut,
    Rejected,
    Cancelled,
    RateLimited,
    ServiceUnavailable,
}
type TraceCheckpoint = dyn Fn(&[ModelCallAttempt]) -> Result<()> + Send + Sync;
#[derive(Default)]
pub struct ModelCallTrace {
    next_call: std::sync::atomic::AtomicU64,
    entries: std::sync::Mutex<(Vec<ModelCallAttempt>, bool)>,
    checkpoint: std::sync::Mutex<Option<Arc<TraceCheckpoint>>>,
    // Serialize mutation + callback, preserving durable snapshot order while
    // releasing entries before crossing the application's persistence boundary.
    checkpoint_serial: std::sync::Mutex<()>,
}
impl ModelCallTrace {
    /// Install once before any attempts. The callback receives a bounded complete
    /// copy, outside the entries lock, before a request and after its outcome.
    /// Consume the slice directly: do not call snapshot(), mutate the trace or
    /// install hooks from this callback (the checkpoint sequence is serialized).
    /// Errors are sticky and never expose callback/provider error text.
    pub fn set_checkpoint<F>(&self, checkpoint: F) -> Result<()>
    where
        F: Fn(&[ModelCallAttempt]) -> Result<()> + Send + Sync + 'static,
    {
        let _serial = self
            .checkpoint_serial
            .lock()
            .map_err(|_| anyhow::anyhow!("model trace unavailable"))?;
        let entries = self
            .entries
            .lock()
            .map_err(|_| anyhow::anyhow!("model trace unavailable"))?;
        ensure!(
            entries.0.is_empty() && !entries.1,
            "model trace already started"
        );
        let mut hook = self
            .checkpoint
            .lock()
            .map_err(|_| anyhow::anyhow!("model trace unavailable"))?;
        ensure!(hook.is_none(), "model trace checkpoint already installed");
        *hook = Some(Arc::new(checkpoint));
        Ok(())
    }
    pub fn snapshot(&self) -> Result<Vec<ModelCallAttempt>> {
        // A concurrent owner must not observe an outcome before its durable
        // callback commits (or marks the trace failed). Hooks consume their slice.
        let _serial = self
            .checkpoint_serial
            .lock()
            .map_err(|_| anyhow::anyhow!("model trace unavailable"))?;
        let entries = self
            .entries
            .lock()
            .map_err(|_| anyhow::anyhow!("model trace unavailable"))?;
        ensure!(!entries.1, "model trace capacity or checkpoint failed");
        Ok(entries.0.clone())
    }
    fn record(&self, entry: ModelCallAttempt) -> Result<()> {
        let _serial = self
            .checkpoint_serial
            .lock()
            .map_err(|_| anyhow::anyhow!("model trace unavailable"))?;
        let saved = {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| anyhow::anyhow!("model trace unavailable"))?;
            ensure!(!entries.1, "model trace capacity or checkpoint failed");
            if let Some(previous) = entries
                .0
                .iter_mut()
                .find(|v| v.call == entry.call && v.attempt == entry.attempt)
            {
                if previous.outcome != ModelCallOutcome::Started
                    || entry.outcome == ModelCallOutcome::Started
                    || previous.profile_id != entry.profile_id
                {
                    entries.1 = true;
                    anyhow::bail!("invalid model trace transition");
                }
                *previous = entry;
            } else if entry.outcome == ModelCallOutcome::Started
                && entry.call > 0
                && entry.attempt > 0
                && entries.0.len() < 1024
            {
                entries.0.push(entry);
            } else {
                entries.1 = true;
                anyhow::bail!("model trace capacity or transition failed");
            }
            entries.0.clone()
        };
        let hook = self
            .checkpoint
            .lock()
            .map_err(|_| anyhow::anyhow!("model trace unavailable"))?
            .clone();
        if let Some(hook) = hook {
            let committed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hook(&saved)));
            if !matches!(committed, Ok(Ok(()))) {
                if let Ok(mut entries) = self.entries.lock() {
                    entries.1 = true;
                }
                anyhow::bail!("model trace checkpoint failed");
            }
        }
        Ok(())
    }
}

struct TraceAttempt<'a> {
    trace: &'a ModelCallTrace,
    entry: ModelCallAttempt,
    finished: bool,
}
impl<'a> TraceAttempt<'a> {
    fn begin(trace: &'a ModelCallTrace, entry: ModelCallAttempt) -> Result<Self, LlmError> {
        trace
            .record(entry.clone())
            .map_err(|_| LlmError::EvidenceUnavailable)?;
        Ok(Self {
            trace,
            entry,
            finished: false,
        })
    }
    fn finish(&mut self, outcome: ModelCallOutcome) -> Result<(), LlmError> {
        self.finished = true;
        self.entry.outcome = outcome;
        self.trace
            .record(self.entry.clone())
            .map_err(|_| LlmError::EvidenceUnavailable)
    }
}
impl Drop for TraceAttempt<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.entry.outcome = ModelCallOutcome::Cancelled;
            // Drop cannot return a persistence error. record makes it sticky so
            // the owner cannot subsequently snapshot/complete successfully.
            let _ = self.trace.record(self.entry.clone());
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
            let mut attempt = self
                .trace
                .as_ref()
                .map(|trace| {
                    TraceAttempt::begin(
                        trace,
                        ModelCallAttempt {
                            call,
                            attempt: (index + 1) as u8,
                            profile_id: id.clone(),
                            outcome: ModelCallOutcome::Started,
                        },
                    )
                })
                .transpose()?;
            // Synchronous durable admission consumes the same total budget.
            // Do not even poll a provider if its checkpoint exhausted it.
            if tokio::time::Instant::now() >= deadline {
                if let Some(attempt) = &mut attempt {
                    attempt.finish(ModelCallOutcome::TimedOut)?;
                }
                return Err(LlmError::ModelFailure("model route timed out"));
            }
            let response = tokio::time::timeout_at(
                deadline.min(tokio::time::Instant::now() + candidate_budget),
                client.chat_with_options(messages, options.clone()),
            )
            .await;
            if let Some(attempt) = &mut attempt {
                let outcome = match &response {
                    Ok(Ok(_)) => ModelCallOutcome::Succeeded,
                    Ok(Err(LlmError::RateLimited)) => ModelCallOutcome::RateLimited,
                    Ok(Err(LlmError::ServiceUnavailable)) => ModelCallOutcome::ServiceUnavailable,
                    Ok(Err(LlmError::ModelFailure(_))) | Ok(Err(LlmError::Request(_))) => {
                        ModelCallOutcome::TransportFailure
                    }
                    Err(_) => ModelCallOutcome::TimedOut,
                    Ok(Err(_)) => ModelCallOutcome::Rejected,
                };
                attempt.finish(outcome)?;
            }
            match response {
                Ok(Ok(result)) => {
                    tracing::info!(provider_id = %id, "model request completed");
                    return Ok(result);
                }
                Ok(Err(LlmError::ModelFailure(_)))
                | Ok(Err(LlmError::Request(_)))
                | Ok(Err(LlmError::RateLimited))
                | Ok(Err(LlmError::ServiceUnavailable))
                | Err(_) => {
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
            default_max_tokens: 4096,
        }))
    }
}
struct OpenAiCompatibleClient {
    profile: ModelProfile,
    client: reqwest::Client,
    default_max_tokens: u32,
}
#[derive(Deserialize)]
struct Completion {
    choices: Vec<CompletionChoice>,
    #[serde(default)]
    object: Option<String>,
    #[serde(default)]
    error: Option<serde_json::Value>,
}
#[derive(Deserialize)]
struct CompletionChoice {
    message: serde_json::Value,
    // Required: a missing reason cannot prove that a provider completed output.
    finish_reason: String,
}
fn completion_output(bytes: &[u8], structured: bool) -> Result<String, LlmError> {
    let rejected = || LlmError::Api("model response is incomplete or unsupported".into());
    let mut completion: Completion = serde_json::from_slice(bytes).map_err(|_| rejected())?;
    if completion.choices.len() != 1
        || completion.error.is_some()
        || completion
            .object
            .as_deref()
            .is_some_and(|object| object != "chat.completion")
    {
        return Err(rejected());
    }
    let choice = completion.choices.pop().unwrap();
    let message = choice.message.as_object().ok_or_else(rejected)?;
    if choice.finish_reason != "stop"
        || message.get("role").and_then(serde_json::Value::as_str) != Some("assistant")
        || message.keys().any(|key| {
            !matches!(
                key.as_str(),
                "role"
                    | "content"
                    | "refusal"
                    | "tool_calls"
                    | "function_call"
                    | "audio"
                    | "annotations"
            )
        })
        || ["refusal", "tool_calls", "function_call", "audio"]
            .iter()
            .any(|key| message.get(*key).is_some_and(|value| !value.is_null()))
        || message
            .get("annotations")
            .is_some_and(|value| !value.is_null() && !value.as_array().is_some_and(Vec::is_empty))
    {
        return Err(rejected());
    }
    let content = message
        .get("content")
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(rejected)?;
    if structured && serde_json::from_str::<serde_json::Value>(content).is_err() {
        return Err(LlmError::Api("invalid model structured output".into()));
    }
    Ok(content.into())
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
        let mut body = serde_json::json!({"model":self.profile.model,"messages":conversation,"max_tokens":options.max_tokens.unwrap_or(self.default_max_tokens)});
        if let Some(temperature) = options.temperature {
            body["temperature"] = serde_json::json!(temperature);
        }
        let structured = options.json_schema.is_some();
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
        let status = response.status();
        if status.as_u16() == 429 {
            return Err(LlmError::RateLimited);
        }
        if status.is_server_error() {
            return Err(LlmError::ServiceUnavailable);
        }
        if !status.is_success() {
            return Err(LlmError::Api("model request rejected".into()));
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
                return Err(LlmError::Api("model response exceeds limit".into()));
            }
            bytes.extend_from_slice(&chunk);
        }
        completion_output(&bytes, structured)
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
    fn explicit_execution_profile_requires_current_visual_authorized_registration() {
        let dir = tempfile::tempdir().unwrap();
        let registry = ModelRegistry::new(dir.path().join("models.json"));
        let mut selected = profile("chosen");
        selected.capabilities.push("vision".into());
        selected.midscene_family = Some("qwen2.5-vl".into());
        registry.upsert(0, selected.clone(), false).unwrap();
        let frozen = registry.resolve_execution_profile("chosen", 1).unwrap();
        assert_eq!(frozen.candidates().len(), 1);
        assert_eq!(frozen.candidates()[0].model, "model-chosen");
        assert!(registry.resolve_execution_profile("chosen", 0).is_err());
        assert!(registry.resolve_execution_profile("missing", 1).is_err());
        selected.enabled = false;
        registry.upsert(1, selected.clone(), false).unwrap();
        assert!(registry.resolve_execution_profile("chosen", 2).is_err());
        selected.enabled = true;
        selected.allow_remote = false;
        registry.upsert(2, selected.clone(), false).unwrap();
        assert!(registry.resolve_execution_profile("chosen", 3).is_err());
        selected.allow_remote = true;
        selected.midscene_family = None;
        registry.upsert(3, selected.clone(), false).unwrap();
        assert!(registry.resolve_execution_profile("chosen", 4).is_err());
        selected.midscene_family = Some("qwen2.5-vl".into());
        selected.capabilities = vec!["text".into()];
        registry.upsert(4, selected, false).unwrap();
        assert!(registry.resolve_execution_profile("chosen", 5).is_err());
        assert_eq!(frozen.candidates()[0].model, "model-chosen");
        assert_eq!(frozen.candidates()[0].api_key, "private-test-key");
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
                let response = serde_json::json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":expected}}]}).to_string();
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
        for call in 1..=1024 {
            trace
                .record(ModelCallAttempt {
                    call,
                    attempt: 1,
                    profile_id: "fixture".into(),
                    outcome: ModelCallOutcome::Started,
                })
                .unwrap();
        }
        assert!(trace
            .record(ModelCallAttempt {
                call: 1025,
                attempt: 1,
                profile_id: "fixture".into(),
                outcome: ModelCallOutcome::Started
            })
            .is_err());
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

    async fn completion_server(
        status: u16,
        body: String,
        delay: Duration,
    ) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0u8; 4096];
                let n = stream.read(&mut chunk).await.unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let len: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .unwrap()
                        .trim()
                        .parse()
                        .unwrap();
                    if bytes.len() >= end + 4 + len {
                        assert!(headers.starts_with("post /v1/chat/completions "));
                        assert!(headers.contains("authorization: bearer private-test-key"));
                        count.fetch_add(1, Ordering::SeqCst);
                        break;
                    }
                }
            }
            tokio::time::sleep(delay).await;
            if status == 0 {
                return;
            } // A real peer disconnect, not a simulated error.
            let response = format!("HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = stream.write_all(response.as_bytes()).await;
        });
        (endpoint, calls, task)
    }
    fn complete(text: &str) -> serde_json::Value {
        serde_json::json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":text,"refusal":null,"annotations":[]}}]})
    }
    #[tokio::test]
    async fn completion_wire_rejections_never_fall_back_and_never_echo_provider_body() {
        let mut responses = vec![
            (400, "private-test-key".into()),
            (401, "private-test-key".into()),
            (403, "private-test-key".into()),
            (404, "private-test-key".into()),
            (408, "private-test-key".into()),
            (422, "private-test-key".into()),
            (302, "private-test-key".into()),
            (200, "bad private-test-key".into()),
            (200, "x".repeat(4 * 1024 * 1024 + 1)),
        ];
        for reason in [
            "length",
            "content_filter",
            "tool_calls",
            "function_call",
            "unknown",
            "",
        ] {
            let mut value = complete("private-test-key");
            value["choices"][0]["finish_reason"] = serde_json::json!(reason);
            responses.push((200, value.to_string()));
        }
        for field in [
            "missing",
            "null",
            "refusal",
            "tool_calls",
            "function_call",
            "role",
            "content",
            "unknown",
            "object",
            "error",
            "annotations",
            "audio",
        ] {
            let mut value = complete("private-test-key");
            match field {
                "missing" => {
                    value["choices"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove("finish_reason");
                }
                "null" => value["choices"][0]["finish_reason"] = serde_json::Value::Null,
                "role" => value["choices"][0]["message"]["role"] = serde_json::json!("user"),
                "content" => {
                    value["choices"][0]["message"]["content"] =
                        serde_json::json!([{"type":"unknown","text":"private-test-key"}])
                }
                "unknown" => {
                    value["choices"][0]["message"]["unknown"] =
                        serde_json::json!("private-test-key")
                }
                "object" => value["object"] = serde_json::json!("chat.completion.chunk"),
                "error" => value["error"] = serde_json::json!("private-test-key"),
                other => {
                    value["choices"][0]["message"][other] = serde_json::json!("private-test-key")
                }
            }
            responses.push((200, value.to_string()));
        }
        responses.push((200, serde_json::json!({"choices":[]}).to_string()));
        for (status, body) in responses {
            let (endpoint, calls, server) = completion_server(status, body, Duration::ZERO).await;
            let mut p = profile("primary");
            p.base_url = endpoint;
            let fallback_calls = Arc::new(AtomicUsize::new(0));
            let trace = Arc::new(ModelCallTrace::default());
            let route = RegisteredClient {
                clients: vec![
                    ("primary".into(), direct_completion_client(&p)),
                    (
                        "fallback".into(),
                        ArcLlmClient::new(MockClient {
                            id: "fallback".into(),
                            calls: fallback_calls.clone(),
                            business_failure: false,
                        }),
                    ),
                ],
                capabilities: BTreeSet::from(["text".into()]),
                trace: Some(trace.clone()),
            };
            let error = route
                .chat_with_budget(
                    &[Message::user("hello")],
                    ChatOptions::default(),
                    Duration::from_secs(2),
                )
                .await
                .err()
                .unwrap();
            server.await.unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(fallback_calls.load(Ordering::SeqCst), 0);
            let entries = trace.snapshot().unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].outcome, ModelCallOutcome::Rejected);
            assert!(
                !format!("{error}{}", serde_json::to_string(&entries).unwrap())
                    .contains("private-test-key")
            );
        }
    }
    // A disconnect must reach reqwest directly: system proxies can translate a
    // closed peer into HTTP 502 and thereby change the observed failure class.
    // This only isolates fixture transport; the production adapter is unchanged.
    pub(super) fn direct_completion_client(profile: &ModelProfile) -> ArcLlmClient {
        ArcLlmClient::new(OpenAiCompatibleClient {
            profile: profile.clone(),
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            default_max_tokens: 4096,
        })
    }
    fn legacy_provider(endpoint: String, model: &str) -> crate::llm::ProviderConfig {
        crate::llm::ProviderConfig {
            provider: "anthropic_messages".into(), // A label cannot change the legacy wire.
            base_url: endpoint,
            api_key: "private-test-key".into(),
            model: model.into(),
        }
    }
    #[test]
    fn legacy_generation_freezes_bounded_safe_candidates_and_rejects_credentials_in_metadata() {
        let mut primary = legacy_provider("http://127.0.0.1:9/v1".into(), "original");
        let frozen = LegacyGenerationRoute::freeze(
            primary.clone(),
            Some(legacy_provider(
                "http://127.0.0.1:10/v1".into(),
                "secondary",
            )),
        )
        .unwrap();
        primary.model = "changed".into();
        primary.api_key = "changed-key".into();
        let snapshot = frozen.snapshot();
        assert_eq!(snapshot["model"], "original");
        assert_eq!(snapshot["candidates"][0]["id"], "legacy-primary");
        assert_eq!(snapshot["candidates"][1]["id"], "legacy-secondary");
        assert_eq!(snapshot["candidates"][0]["adapter"], "openai_compatible");
        assert_eq!(frozen.candidates()[0].api_key, "private-test-key");
        for private in ["private-test-key", "127.0.0.1", "changed-key"] {
            assert!(!snapshot.to_string().contains(private));
        }
        let mut escaped = legacy_provider("http://127.0.0.1:9/v1".into(), "one");
        escaped.api_key = "quoted-\"credential".into();
        escaped.model = escaped.api_key.clone();
        assert!(LegacyGenerationRoute::freeze(escaped, None).is_err());
        for mutation in 0..4 {
            let mut invalid = legacy_provider("http://127.0.0.1:9/v1".into(), "one");
            match mutation {
                0 => invalid.base_url = "https://example.invalid/v1?key=secret".into(),
                1 => invalid.api_key.clear(),
                2 => invalid.model = "x".repeat(201),
                _ => invalid.model = "private-test-key".into(),
            }
            assert!(LegacyGenerationRoute::freeze(invalid, None).is_err());
        }
    }
    #[tokio::test]
    async fn legacy_generation_uses_completion_wire_and_preserves_token_default_and_override() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for override_tokens in [None, Some(37)] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0; 4096];
                    let size = socket.read(&mut chunk).await.unwrap();
                    assert!(size > 0);
                    bytes.extend_from_slice(&chunk[..size]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                        let length: usize = headers
                            .lines()
                            .find_map(|h| h.strip_prefix("content-length:"))
                            .unwrap()
                            .trim()
                            .parse()
                            .unwrap();
                        if bytes.len() >= end + 4 + length {
                            assert!(headers.starts_with("post /v1/chat/completions "));
                            let body: serde_json::Value =
                                serde_json::from_slice(&bytes[end + 4..]).unwrap();
                            let response = complete("ok").to_string();
                            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).as_bytes()).await.unwrap();
                            return body;
                        }
                    }
                }
            });
            let frozen =
                LegacyGenerationRoute::freeze(legacy_provider(endpoint, "original"), None).unwrap();
            let (client, trace) = frozen.build_client_with_trace().unwrap();
            let options = ChatOptions {
                max_tokens: override_tokens,
                ..ChatOptions::default()
            };
            assert_eq!(
                client
                    .chat_with_options(&[Message::user("hello")], options)
                    .await
                    .unwrap(),
                "ok"
            );
            let body = server.await.unwrap();
            assert_eq!(body["model"], "original");
            assert_eq!(body["max_tokens"], override_tokens.unwrap_or(1024));
            assert_eq!(
                trace.snapshot().unwrap(),
                vec![ModelCallAttempt {
                    call: 1,
                    attempt: 1,
                    profile_id: "legacy-primary".into(),
                    outcome: ModelCallOutcome::Succeeded,
                }]
            );
        }
    }
    #[tokio::test]
    async fn completion_wire_retryable_failures_use_only_approved_fallback_within_budget() {
        for status in [0, 429, 500, 503, 529] {
            let (first, calls, first_server) =
                completion_server(status, "private-test-key".into(), Duration::ZERO).await;
            let (second, fallback_calls, second_server) =
                completion_server(200, complete("ok").to_string(), Duration::ZERO).await;
            let mut p = profile("primary");
            p.base_url = first;
            let mut f = profile("fallback");
            f.base_url = second;
            let trace = Arc::new(ModelCallTrace::default());
            let route = RegisteredClient {
                clients: vec![
                    ("primary".into(), direct_completion_client(&p)),
                    ("fallback".into(), direct_completion_client(&f)),
                ],
                capabilities: BTreeSet::from(["text".into()]),
                trace: Some(trace.clone()),
            };
            assert_eq!(
                route
                    .chat_with_budget(
                        &[Message::user("hi")],
                        ChatOptions::default(),
                        Duration::from_secs(2)
                    )
                    .await
                    .unwrap(),
                "ok"
            );
            first_server.await.unwrap();
            second_server.await.unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(fallback_calls.load(Ordering::SeqCst), 1);
            let entries = trace.snapshot().unwrap();
            assert_eq!(entries.len(), 2);
            assert_eq!(
                entries[0].outcome,
                match status {
                    0 => ModelCallOutcome::TransportFailure,
                    429 => ModelCallOutcome::RateLimited,
                    _ => ModelCallOutcome::ServiceUnavailable,
                }
            );
            assert_eq!(entries[1].outcome, ModelCallOutcome::Succeeded);
            assert!(!serde_json::to_string(&entries)
                .unwrap()
                .contains("private-test-key"));
        }
        let (endpoint, calls, server) =
            completion_server(200, complete("late").to_string(), Duration::from_secs(1)).await;
        let mut p = profile("slow");
        p.base_url = endpoint;
        let trace = Arc::new(ModelCallTrace::default());
        let route = RegisteredClient {
            clients: vec![("slow".into(), direct_completion_client(&p))],
            capabilities: BTreeSet::from(["text".into()]),
            trace: Some(trace.clone()),
        };
        let start = std::time::Instant::now();
        assert!(route
            .chat_with_budget(
                &[Message::user("hi")],
                ChatOptions::default(),
                Duration::from_millis(40)
            )
            .await
            .is_err());
        assert!(start.elapsed() < Duration::from_millis(500));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            trace.snapshot().unwrap()[0].outcome,
            ModelCallOutcome::TimedOut
        );
        server.abort();
    }
    #[test]
    fn completion_requires_complete_single_text_and_valid_structured_json() {
        assert_eq!(
            completion_output(&serde_json::to_vec(&complete("ok")).unwrap(), false).unwrap(),
            "ok"
        );
        assert!(completion_output(
            &serde_json::to_vec(&complete("private-test-key")).unwrap(),
            true
        )
        .is_err());
        assert!(completion_output(&serde_json::to_vec(&complete(" ")).unwrap(), false).is_err());
        let mut many = complete("ok");
        let extra = many["choices"][0].clone();
        many["choices"].as_array_mut().unwrap().push(extra);
        assert!(completion_output(&serde_json::to_vec(&many).unwrap(), false).is_err());
        assert_eq!(
            completion_output(
                &serde_json::to_vec(&complete("{\"ok\":true}")).unwrap(),
                true
            )
            .unwrap(),
            "{\"ok\":true}"
        );
    }

    fn traced_completion(
        endpoint: String,
        trace: Arc<ModelCallTrace>,
        fallback_calls: Arc<AtomicUsize>,
    ) -> RegisteredClient {
        let mut primary = profile("primary");
        primary.base_url = endpoint;
        RegisteredClient {
            clients: vec![
                ("primary".into(), direct_completion_client(&primary)),
                (
                    "fallback".into(),
                    ArcLlmClient::new(MockClient {
                        id: "fallback".into(),
                        calls: fallback_calls,
                        business_failure: false,
                    }),
                ),
            ],
            capabilities: BTreeSet::from(["text".into()]),
            trace: Some(trace),
        }
    }
    #[tokio::test]
    async fn trace_checkpoints_admit_before_http_and_update_cancelled_in_place_on_drop() {
        let (endpoint, calls, server) =
            completion_server(200, complete("late").to_string(), Duration::from_secs(1)).await;
        let trace = Arc::new(ModelCallTrace::default());
        let saved = Arc::new(std::sync::Mutex::new(Vec::<Vec<ModelCallAttempt>>::new()));
        let sink = saved.clone();
        let observed_calls = calls.clone();
        let weak_trace = Arc::downgrade(&trace);
        trace
            .set_checkpoint(move |entries| {
                // The callback can read without the entries lock being held. It
                // receives its own complete copy and never needs to snapshot it.
                assert!(weak_trace.upgrade().unwrap().entries.try_lock().is_ok());
                if entries.last().unwrap().outcome == ModelCallOutcome::Started {
                    assert_eq!(observed_calls.load(Ordering::SeqCst), 0);
                }
                sink.lock().unwrap().push(entries.to_vec());
                Ok(())
            })
            .unwrap();
        let fallback_calls = Arc::new(AtomicUsize::new(0));
        let route = traced_completion(endpoint, trace.clone(), fallback_calls.clone());
        let messages = [Message::user("private input")];
        let mut future = Box::pin(route.chat_with_budget(
            &messages,
            ChatOptions::default(),
            Duration::from_secs(2),
        ));
        tokio::select! {
            _ = &mut future => panic!("delayed provider completed unexpectedly"),
            _ = async { while calls.load(Ordering::SeqCst)==0 { tokio::task::yield_now().await; } } => {}
        }
        assert_eq!(
            trace.snapshot().unwrap()[0].outcome,
            ModelCallOutcome::Started
        );
        drop(future);
        assert_eq!(fallback_calls.load(Ordering::SeqCst), 0);
        let events = saved.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0][0].outcome, ModelCallOutcome::Started);
        assert_eq!(events[1][0].outcome, ModelCallOutcome::Cancelled);
        assert_eq!(trace.snapshot().unwrap().len(), 1);
        assert!(trace.set_checkpoint(|_| Ok(())).is_err());
        assert!(!serde_json::to_string(&*events).unwrap().contains("private"));
        server.abort();
    }
    #[tokio::test]
    async fn trace_checkpoint_failures_stop_before_request_or_fallback_and_stay_observable() {
        for fail_on in [
            ModelCallOutcome::Started,
            ModelCallOutcome::Succeeded,
            ModelCallOutcome::RateLimited,
        ] {
            let status = if fail_on == ModelCallOutcome::RateLimited {
                429
            } else {
                200
            };
            let (endpoint, calls, server) =
                completion_server(status, complete("ok").to_string(), Duration::ZERO).await;
            let trace = Arc::new(ModelCallTrace::default());
            let fail = fail_on.clone();
            trace
                .set_checkpoint(move |entries| {
                    if entries.last().unwrap().outcome == fail {
                        anyhow::bail!("private callback credential/body");
                    }
                    Ok(())
                })
                .unwrap();
            assert!(trace.set_checkpoint(|_| Ok(())).is_err());
            let fallback_calls = Arc::new(AtomicUsize::new(0));
            let route = traced_completion(endpoint, trace.clone(), fallback_calls.clone());
            let error = route
                .chat_with_budget(
                    &[Message::user("hi")],
                    ChatOptions::default(),
                    Duration::from_secs(2),
                )
                .await
                .err()
                .unwrap();
            assert!(matches!(error, LlmError::EvidenceUnavailable));
            assert!(!error.to_string().contains("private"));
            assert_eq!(fallback_calls.load(Ordering::SeqCst), 0);
            assert_eq!(
                calls.load(Ordering::SeqCst),
                usize::from(fail_on != ModelCallOutcome::Started)
            );
            assert!(trace.snapshot().is_err());
            server.abort();
        }
        let trace = Arc::new(ModelCallTrace::default());
        trace
            .set_checkpoint(|entries| {
                if entries[0].outcome == ModelCallOutcome::Cancelled {
                    anyhow::bail!("private failed drop");
                }
                Ok(())
            })
            .unwrap();
        let entry = ModelCallAttempt {
            call: 1,
            attempt: 1,
            profile_id: "fixture".into(),
            outcome: ModelCallOutcome::Started,
        };
        let guard = TraceAttempt::begin(&trace, entry).unwrap();
        drop(guard);
        assert!(trace.snapshot().is_err());
    }

    #[tokio::test]
    async fn trace_durable_admission_consumes_deadline_without_polling_provider() {
        let (endpoint, calls, server) =
            completion_server(200, complete("unused").to_string(), Duration::ZERO).await;
        let trace = Arc::new(ModelCallTrace::default());
        trace
            .set_checkpoint(|entries| {
                if entries[0].outcome == ModelCallOutcome::Started {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Ok(())
            })
            .unwrap();
        let fallback_calls = Arc::new(AtomicUsize::new(0));
        let route = traced_completion(endpoint, trace.clone(), fallback_calls.clone());
        assert!(route
            .chat_with_budget(
                &[Message::user("hi")],
                ChatOptions::default(),
                Duration::from_millis(5)
            )
            .await
            .is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(fallback_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            trace.snapshot().unwrap()[0].outcome,
            ModelCallOutcome::TimedOut
        );
        server.abort();
    }
}
