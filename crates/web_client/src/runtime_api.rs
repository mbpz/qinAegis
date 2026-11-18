//! Native runtime maintenance. All client references are server-issued identities.
use crate::app::AppState;
use anyhow::{ensure, Context, Result};
use qin_aegis_core::{
    runtime_install::Installer,
    runtime_transfer::{TransferStore, CHUNK_BYTES},
    LocalStorage,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub(crate) fn transfers() -> Result<TransferStore> {
    TransferStore::new(&LocalStorage::base_path())
}
fn installer() -> Result<Installer> {
    Installer::new(&LocalStorage::base_path())
}
fn selection() -> &'static str {
    if std::env::var_os("QINAEGIS_RUNTIME_DIR").is_some() {
        "external"
    } else if std::env::var_os("QINAEGIS_DEV_SANDBOX").is_some() {
        "developer"
    } else {
        "managed"
    }
}
pub(crate) fn recover() -> Result<()> {
    let installation = installer().and_then(|i| i.recover(&AtomicBool::new(false)));
    let transfer = transfers().and_then(|s| s.recover());
    match (installation, transfer) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(e), Ok(())) | (Ok(()), Err(e)) => Err(e),
        (Err(e), Err(t)) => Err(anyhow::anyhow!("{e}; transfer recovery: {t}")),
    }
}
fn normalize_preview(raw: &Value, state: &Value) -> Value {
    json!({"preview_token":raw["token"],"expected_revision":raw["revision"].as_u64().map(|n|n.to_string()),
        "operation":raw["kind"],"candidate":{"version":raw["version"],"archive_sha256":raw["archive_sha256"],"source_commit":raw["source_commit"]},
        "current":state["current"],"previous":state["previous"],"expires_at":raw["expires_at"].as_u64().map(|n|n.to_string()),
        "discard_previous_required":raw["discard_previous_required"]})
}
fn preview_result(preview: qin_aegis_core::runtime_install::Preview) -> Result<Value> {
    let state = installer()?.inspect()?;
    Ok(normalize_preview(&serde_json::to_value(preview)?, &state))
}
pub(crate) fn facts() -> Value {
    let mut error = None;
    let state = match installer().and_then(|i| i.inspect()) {
        Ok(v) => v,
        Err(e) => {
            error = Some(e.to_string());
            json!({})
        }
    };
    if state["recovery_required"] == true {
        error=Some("Runtime recovery is required; engine tasks are disabled. Asset and report management remain available.".into());
    }
    let upload=match transfers().and_then(|s|s.inspect()) {Ok(v)=>v.map(|u|json!({"upload_id":u.upload_id,"size":u.size,"received":u.received,"claimed":u.claimed})),Err(e)=>{error.get_or_insert(e.to_string());None}};
    let preview = state
        .get("preview")
        .filter(|p| !p.is_null())
        .map(|p| normalize_preview(p, &state));
    json!({"current":state.get("current"),"previous":state.get("previous"),"selection":selection(),"blocked_reason":error,
        "unmanaged_current":state["unmanaged_current"],"unmanaged_previous":state["unmanaged_previous"],"preview":preview,"upload":upload,
        "downloads":{"available":false,"reason":"Check the publisher's stable releases explicitly. No network request is made on page load."}})
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Begin {
    size: u64,
    expected_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UploadId {
    upload_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Token {
    preview_token: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Confirm {
    preview_token: String,
    expected_revision: String,
    discard_previous: bool,
    approved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DownloadId {
    asset_id: String,
}
fn params<T: serde::de::DeserializeOwned>(p: &Value) -> Result<T> {
    serde_json::from_value(p.clone()).context("invalid runtime maintenance parameters")
}
fn empty(p: &Value) -> Result<()> {
    ensure!(
        p.as_object().is_some_and(|m| m.is_empty()),
        "unexpected runtime parameters"
    );
    Ok(())
}
pub(crate) fn handles(method: &str) -> bool {
    matches!(
        method,
        "getRuntimeMaintenance"
            | "beginRuntimeUpload"
            | "abortRuntimeUpload"
            | "prepareRuntimeInstall"
            | "prepareRuntimeRollback"
            | "confirmRuntimeInstall"
            | "confirmRuntimeRollback"
            | "discardRuntimePreview"
            | "listRuntimeDownloads"
            | "prepareRuntimeDownload"
    )
}
pub(crate) fn dispatch(app: &mut AppState, method: &str, p: &Value) -> Result<Value> {
    match method {
        "getRuntimeMaintenance" => {
            empty(p)?;
            Ok(facts())
        }
        "beginRuntimeUpload" => {
            let b: Begin = params(p)?;
            ensure!(
                !app.runtime_maintenance_active(),
                "Wait for runtime maintenance cleanup before uploading"
            );
            Ok(serde_json::to_value(transfers()?.begin(
                &crate::server::random_token()?,
                b.size,
                &b.expected_sha256,
            )?)?)
        }
        "abortRuntimeUpload" => {
            let b: UploadId = params(p)?;
            ensure!(
                !app.runtime_maintenance_active(),
                "Stop verification and wait for cleanup before discarding the upload"
            );
            transfers()?.abort(&b.upload_id)?;
            Ok(json!({"ok":true}))
        }
        "prepareRuntimeInstall" => {
            let b: UploadId = params(p)?;
            let token = crate::server::random_token()?;
            app.start_runtime_maintenance(move |cancel, report| {
                let store = transfers()?;
                let (path, sha) = store.claim(&b.upload_id)?;
                report("Verifying archive integrity and staging its contents.\n");
                let outcome = (|| {
                    installer()?
                        .prepare(&path, &sha, &token, &cancel)
                        .and_then(preview_result)
                })();
                let release = store.release(&b.upload_id);
                match (outcome, release) {
                    (Ok(v), Ok(())) => {
                        store.abort(&b.upload_id)?;
                        report("Verified candidate staged; review and approve the preview.\n");
                        Ok(v)
                    }
                    (Err(e), Ok(())) | (Ok(_), Err(e)) => Err(e),
                    (Err(e), Err(r)) => Err(anyhow::anyhow!("{e}; upload retained: {r}")),
                }
            })
        }
        "prepareRuntimeRollback" => {
            empty(p)?;
            let token = crate::server::random_token()?;
            app.start_runtime_maintenance(move |cancel, report| {
                report("Verifying the previous runtime for rollback.\n");
                let value = preview_result(installer()?.prepare_rollback(&token, &cancel)?)?;
                report("Rollback verified; review and approve the preview.\n");
                Ok(value)
            })
        }
        "discardRuntimePreview" => {
            let b: Token = params(p)?;
            app.start_runtime_maintenance(move |cancel, report| {
                report("Verifying and discarding the pending preview.\n");
                installer()?.discard_preview(&b.preview_token, &cancel)?;
                report("Pending preview discarded.\n");
                Ok(facts())
            })
        }
        "confirmRuntimeInstall" | "confirmRuntimeRollback" => {
            let b: Confirm = params(p)?;
            ensure!(b.approved, "explicit runtime switch approval required");
            let expected_kind = if method == "confirmRuntimeInstall" {
                "install"
            } else {
                "rollback"
            };
            app.start_runtime_maintenance(move |cancel, report| {
                let i = installer()?;
                let state = i.inspect()?;
                let pending = &state["preview"];
                ensure!(
                    state["revision"].as_u64().map(|n| n.to_string()).as_deref()
                        == Some(b.expected_revision.as_str())
                        && pending["token"] == b.preview_token
                        && pending["kind"] == expected_kind,
                    "Runtime preview changed; prepare and review again"
                );
                report("Rechecking installed contents and the approved change before switching.\n");
                let mut checks = 0;
                i.activate(&b.preview_token, b.discard_previous, &cancel, |path, _| {
                    checks += 1;
                    report(if checks == 1 {
                        "Checking the candidate engine before switching.\n"
                    } else {
                        "Checking the promoted engine after switching.\n"
                    });
                    crate::environment::healthcheck_at(path, cancel.clone())
                })?;
                report("Runtime change committed; transaction cleanup finished.\n");
                Ok(facts())
            })
        }
        "listRuntimeDownloads" => {
            empty(p)?;
            app.start_maintenance_value("runtimeCatalog",|cancel|{
            let rt=tokio::runtime::Builder::new_current_thread().enable_all().build()?;
            rt.block_on(async{let assets=catalog(cancel).await?;Ok(json!({"releases":assets.iter().map(|a|json!({"asset_id":a.id,"version":a.version,"name":a.name,"size":a.size})).collect::<Vec<_>>(),"reason":if assets.is_empty(){"No compatible stable runtime archive and checksum have been published."}else{"Select a version and explicitly allow its download."}}))})
        })
        }
        "prepareRuntimeDownload" => {
            let b: DownloadId = params(p)?;
            let token = crate::server::random_token()?;
            app.start_runtime_maintenance(move |cancel, report| {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                let (upload, version) =
                    rt.block_on(download(&b.asset_id, cancel.clone(), report.clone()))?;
                let store = transfers()?;
                let (path, sha) = store.claim(&upload)?;
                let outcome = (|| {
                    let i = installer()?;
                    report("Verifying downloaded archive integrity and staging its contents.\n");
                    let preview = i.prepare(&path, &sha, &token, &cancel)?;
                    if preview.version != version.trim_start_matches('v') {
                        i.discard_preview(&token, &AtomicBool::new(false))?;
                        anyhow::bail!("Runtime manifest version differs from the selected release");
                    }
                    preview_result(preview)
                })();
                let release = store.release(&upload);
                match (outcome, release) {
                    (Ok(v), Ok(())) => {
                        store.abort(&upload)?;
                        report("Verified download staged; review and approve the preview.\n");
                        Ok(v)
                    }
                    (Err(e), Ok(())) | (Ok(_), Err(e)) => Err(e),
                    (Err(e), Err(r)) => Err(anyhow::anyhow!("{e}; upload retained: {r}")),
                }
            })
        }
        _ => anyhow::bail!("unknown runtime method"),
    }
}

#[derive(Clone)]
struct Asset {
    id: String,
    version: String,
    name: String,
    size: u64,
    url: String,
    checksum_url: String,
}
fn host_target() -> String {
    format!(
        "{}-{}",
        if cfg!(target_os = "macos") {
            "darwin"
        } else {
            std::env::consts::OS
        },
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            "arm64"
        } else if cfg!(all(windows, target_arch = "x86_64")) {
            "amd64"
        } else {
            std::env::consts::ARCH
        }
    )
}
fn trusted_asset(url: &str) -> bool {
    url.starts_with("https://github.com/mbpz/qinAegis/releases/download/v2.")
        && url::Url::parse(url).is_ok_and(|u| {
            u.username().is_empty()
                && u.password().is_none()
                && u.port().is_none()
                && u.fragment().is_none()
                && u.query().is_none()
        })
}
fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent("QinAegis/2.0 runtime-maintenance")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}
async fn request(
    client: &reqwest::Client,
    url: &str,
    cancel: Arc<AtomicBool>,
) -> Result<reqwest::Response> {
    let mut url = url::Url::parse(url)?;
    for _ in 0..6 {
        ensure!(
            url.scheme() == "https"
                && url.username().is_empty()
                && url.password().is_none()
                && url.port().is_none()
                && matches!(
                    url.host_str(),
                    Some(
                        "api.github.com"
                            | "github.com"
                            | "release-assets.githubusercontent.com"
                            | "objects.githubusercontent.com"
                    )
                ),
            "runtime download left the publisher's HTTPS hosts"
        );
        let response = tokio::select! {biased;_=crate::environment::cancelled(cancel.clone())=>anyhow::bail!("Runtime download cancelled"),r=tokio::time::timeout(Duration::from_secs(20),client.get(url.clone()).send())=>r.context("runtime publisher response timed out")?.context("runtime publisher unavailable")?};
        if response.status().is_redirection() {
            url = url.join(
                response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .context("runtime download redirect missing")?
                    .to_str()?,
            )?;
            continue;
        }
        ensure!(
            response.status().is_success(),
            "Runtime publisher request failed (HTTP {})",
            response.status().as_u16()
        );
        return Ok(response);
    }
    anyhow::bail!("too many runtime download redirects")
}
async fn chunk(
    response: &mut reqwest::Response,
    cancel: Arc<AtomicBool>,
) -> Result<Option<Vec<u8>>> {
    let result = tokio::select! {biased;_=crate::environment::cancelled(cancel)=>anyhow::bail!("Runtime download cancelled"),r=tokio::time::timeout(Duration::from_secs(15),response.chunk())=>r.context("runtime download stalled")?.context("runtime download interrupted")?};
    Ok(result.map(|b| b.to_vec()))
}
async fn bounded(
    client: &reqwest::Client,
    url: &str,
    limit: usize,
    cancel: Arc<AtomicBool>,
) -> Result<Vec<u8>> {
    let mut response = request(client, url, cancel.clone()).await?;
    ensure!(
        response.content_length().is_none_or(|n| n <= limit as u64),
        "runtime publisher response exceeds limit"
    );
    let mut bytes = Vec::new();
    while let Some(part) = chunk(&mut response, cancel.clone()).await? {
        ensure!(
            part.len() <= limit.saturating_sub(bytes.len()),
            "runtime publisher response exceeds limit"
        );
        bytes.extend(part);
    }
    Ok(bytes)
}
async fn catalog(cancel: Arc<AtomicBool>) -> Result<Vec<Asset>> {
    let client = client()?;
    let bytes = bounded(
        &client,
        "https://api.github.com/repos/mbpz/qinAegis/releases?per_page=20",
        2 * 1024 * 1024,
        cancel,
    )
    .await?;
    let releases: Value = serde_json::from_slice(&bytes)?;
    let releases = releases
        .as_array()
        .context("invalid runtime publisher catalog")?;
    let name = format!("qinaegis-runtime-{}.tar.gz", host_target());
    let checksum = format!("{name}.sha256");
    let mut result = Vec::new();
    for release in releases {
        if release["draft"] != false || release["prerelease"] != false {
            continue;
        }
        let Some(tag) = release["tag_name"].as_str().filter(|s| {
            s.starts_with("v2.")
                && s.len() <= 64
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
        }) else {
            continue;
        };
        let Some(assets) = release["assets"].as_array() else {
            continue;
        };
        let Some(asset) = assets.iter().find(|a| a["name"] == name) else {
            continue;
        };
        let Some(sum) = assets.iter().find(|a| a["name"] == checksum) else {
            continue;
        };
        let (Some(id), Some(size), Some(url), Some(checksum_url)) = (
            asset["id"].as_u64(),
            asset["size"].as_u64(),
            asset["browser_download_url"].as_str(),
            sum["browser_download_url"].as_str(),
        ) else {
            continue;
        };
        let prefix = format!("https://github.com/mbpz/qinAegis/releases/download/{tag}/");
        if size == 0
            || size > qin_aegis_core::runtime_package::MAX_ARCHIVE_BYTES
            || !trusted_asset(url)
            || !trusted_asset(checksum_url)
            || url != format!("{prefix}{name}")
            || checksum_url != format!("{prefix}{checksum}")
        {
            continue;
        }
        result.push(Asset {
            id: id.to_string(),
            version: tag.into(),
            name: name.clone(),
            size,
            url: url.into(),
            checksum_url: checksum_url.into(),
        });
    }
    Ok(result)
}
async fn download(
    id: &str,
    cancel: Arc<AtomicBool>,
    report: crate::app::JobReporter,
) -> Result<(String, String)> {
    ensure!(
        !id.is_empty() && id.len() <= 20 && id.bytes().all(|b| b.is_ascii_digit()),
        "invalid published runtime asset"
    );
    report("Looking up the selected runtime in the trusted publisher catalog.\n");
    let asset = catalog(cancel.clone())
        .await?
        .into_iter()
        .find(|a| a.id == id)
        .context("Published runtime changed or is unavailable; check releases again")?;
    let client = client()?;
    report("Fetching the selected release checksum.\n");
    let checksum =
        String::from_utf8(bounded(&client, &asset.checksum_url, 4096, cancel.clone()).await?)?;
    let fields = checksum.split_whitespace().collect::<Vec<_>>();
    ensure!(
        fields.len() == 2
            && fields[0].len() == 64
            && fields[0].bytes().all(|b| b.is_ascii_hexdigit())
            && fields[1].trim_start_matches('*') == asset.name,
        "invalid publisher runtime checksum"
    );
    let upload = crate::server::random_token()?;
    let store = transfers()?;
    store.begin(&upload, asset.size, fields[0])?;
    let mut response = request(&client, &asset.url, cancel.clone()).await?;
    ensure!(
        response.content_length().is_none_or(|n| n == asset.size),
        "runtime published size changed"
    );
    report(&format!(
        "Downloading {} bytes from the trusted publisher.\n",
        asset.size
    ));
    let mut next_progress = 8 * 1024 * 1024;
    let mut received = 0;
    let mut pending = Vec::with_capacity(CHUNK_BYTES);
    while let Some(part) = chunk(&mut response, cancel.clone()).await? {
        ensure!(
            (part.len() as u64) <= asset.size.saturating_sub(received + pending.len() as u64),
            "runtime download exceeds published size"
        );
        for bytes in part.chunks(CHUNK_BYTES) {
            if pending.len() + bytes.len() > CHUNK_BYTES {
                received = store.append(&upload, received, &pending)?;
                pending.clear();
            }
            pending.extend_from_slice(bytes);
            if pending.len() == CHUNK_BYTES {
                received = store.append(&upload, received, &pending)?;
                pending.clear();
            }
        }
        if received >= next_progress {
            report(&format!(
                "Downloaded {} / {} bytes.\n",
                received, asset.size
            ));
            next_progress = received + 8 * 1024 * 1024;
        }
    }
    if !pending.is_empty() {
        received = store.append(&upload, received, &pending)?;
    }
    ensure!(
        received == asset.size,
        "runtime download ended before published size"
    );
    ensure!(
        !cancel.load(Ordering::Acquire),
        "Runtime download cancelled"
    );
    report(&format!(
        "Downloaded {} / {} bytes.\n",
        received, asset.size
    ));
    Ok((upload, asset.version))
}
