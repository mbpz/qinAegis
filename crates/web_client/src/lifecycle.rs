use crate::{
    app::AppState,
    server::{self, ServerState},
};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

const DEFAULT_PORT: u16 = 19876;
#[derive(Debug, PartialEq)]
struct Options {
    command: String,
    port: Option<u16>,
    no_open: bool,
}
impl Options {
    fn parse(args: Vec<String>) -> Result<Self> {
        let mut options = Self {
            command: "start".into(),
            port: None,
            no_open: false,
        };
        let mut args = args.into_iter().peekable();
        if args.peek().is_some_and(|s| !s.starts_with('-')) {
            options.command = args.next().unwrap();
        }
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--no-open" => options.no_open = true,
                "--port" => {
                    let port = args
                        .next()
                        .context("--port needs a number")?
                        .parse::<u16>()?;
                    ensure!(port > 0, "port must be 1..65535");
                    options.port = Some(port);
                }
                "--help" | "-h" => options.command = "help".into(),
                "--version" | "-V" => options.command = "version".into(),
                _ => bail!("unknown option {arg}"),
            }
        }
        ensure!(
            ["start", "serve", "open", "status", "stop", "version", "help"]
                .contains(&options.command.as_str()),
            "unknown command {}",
            options.command
        );
        ensure!(
            options.port.is_none() || ["start", "serve"].contains(&options.command.as_str()),
            "--port is only valid with start or serve"
        );
        Ok(options)
    }
}

#[derive(Serialize, Deserialize)]
struct Instance {
    origin: String,
    instance_id: String,
    owner_secret: String,
    pid: u32,
    version: String,
}
fn state_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("QINAEGIS_STATE_DIR") {
        let path = PathBuf::from(path);
        ensure!(path.is_absolute(), "QINAEGIS_STATE_DIR must be absolute");
        return Ok(path);
    }
    Ok(dirs::home_dir()
        .context("Cannot locate home directory")?
        .join(".qinAegis/state"))
}
fn private_dir(path: &Path) -> Result<()> {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        ensure!(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "state path must be a real directory"
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true).mode(0o700).create(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(path)?;
    Ok(())
}
fn private_file(path: &Path, fresh: bool) -> Result<File> {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        ensure!(
            !meta.file_type().is_symlink() && meta.is_file(),
            "state file must be a regular file"
        );
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    if fresh {
        options.create_new(true);
    } else {
        options.create(true).truncate(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}
fn save_instance(path: &Path, instance: &Instance) -> Result<()> {
    let tmp = path.with_extension(format!("{}.tmp", instance.instance_id));
    let mut file = private_file(&tmp, true)?;
    file.write_all(&serde_json::to_vec(instance)?)?;
    file.sync_all()?;
    if let Err(error) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(error.into());
    }
    Ok(())
}
fn read_instance(dir: &Path) -> Result<Instance> {
    let path = dir.join("instance.json");
    let meta = std::fs::symlink_metadata(&path)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink() && meta.len() < 4096,
        "invalid local instance file"
    );
    let instance: Instance = serde_json::from_slice(&std::fs::read(path)?)?;
    let url = url::Url::parse(&instance.origin)?;
    ensure!(
        url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.port().is_some()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none(),
        "invalid local instance address"
    );
    Ok(instance)
}
fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()?)
}
async fn healthy(instance: &Instance) -> bool {
    let Ok(client) = client() else {
        return false;
    };
    let Ok(response) = client
        .get(format!("{}/healthz", instance.origin))
        .send()
        .await
    else {
        return false;
    };
    let Ok(body) = response.json::<serde_json::Value>().await else {
        return false;
    };
    body["product"] == "qinaegis"
        && body["api"] == 2
        && body["ready"] == true
        && body["instance_id"] == instance.instance_id
}
async fn browser_url(instance: &Instance) -> Result<String> {
    let response = client()?
        .post(format!("{}/api/v2/launch", instance.origin))
        .header("x-qinaegis-owner", &instance.owner_secret)
        .send()
        .await?
        .error_for_status()?;
    let body: serde_json::Value = response.json().await?;
    let token = body["token"]
        .as_str()
        .context("launch authorization unavailable")?;
    ensure!(
        token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid browser authorization"
    );
    Ok(format!("{}/#token={token}", instance.origin))
}
fn open_browser(url: &str) -> Result<()> {
    let mut cmd;
    #[cfg(target_os = "macos")]
    {
        cmd = Command::new("open");
        cmd.arg(url);
    }
    #[cfg(target_os = "windows")]
    {
        cmd = Command::new("rundll32.exe");
        cmd.args(["url.dll,FileProtocolHandler", url]);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        cmd = Command::new("xdg-open");
        cmd.arg(url);
    }
    let status = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    ensure!(status.success(), "default browser did not open");
    Ok(())
}
async fn show_browser(instance: &Instance) -> Result<()> {
    let url = browser_url(instance).await?;
    if let Err(error) = open_browser(&url) {
        eprintln!("Browser did not open: {error}. Open this one-use URL within 60 seconds:\n{url}");
    }
    Ok(())
}
async fn start(options: &Options, dir: &Path) -> Result<()> {
    if let Ok(instance) = read_instance(dir) {
        if healthy(&instance).await {
            ensure!(
                options
                    .port
                    .is_none_or(|p| instance.origin == format!("http://127.0.0.1:{p}")),
                "service already running on {}; stop it before changing port",
                instance.origin
            );
            println!(
                "QinAegis {} is running at {}",
                instance.version, instance.origin
            );
            if !options.no_open {
                show_browser(&instance).await?;
            }
            return Ok(());
        }
    }
    private_dir(dir)?;
    let log_path = dir.join("service.log");
    let mut log = private_file(&log_path, false)?;
    use std::io::{Seek, SeekFrom};
    log.seek(SeekFrom::End(0))?;
    let mut command = Command::new(std::env::current_exe()?);
    command.args(["serve", "--no-open"]);
    if let Some(port) = options.port {
        command.args(["--port", &port.to_string()]);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Detach the service from the launching console and its Ctrl+C group.
        command.creation_flags(0x00000008 | 0x00000200);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?;
    for _ in 0..50 {
        if let Ok(instance) = read_instance(dir) {
            if healthy(&instance).await {
                ensure!(
                    options
                        .port
                        .is_none_or(|port| instance.origin == format!("http://127.0.0.1:{port}")),
                    "service already running on {}; stop it before changing port",
                    instance.origin
                );
                println!(
                    "QinAegis {} ready at {}\nLog: {}",
                    instance.version,
                    instance.origin,
                    log_path.display()
                );
                if !options.no_open {
                    show_browser(&instance).await?;
                }
                return Ok(());
            }
        }
        // A competing launcher may own the lock before publishing its metadata.
        // Keep polling briefly so both launchers can reuse the same ready instance.
        let _ = child.try_wait()?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if let Some(status) = child.try_wait()? {
        bail!("service exited ({status}); see {}", log_path.display());
    }
    let _ = child.kill();
    let _ = child.wait();
    bail!("service readiness timed out; see {}", log_path.display())
}
async fn serve(options: &Options, dir: &Path) -> Result<()> {
    private_dir(dir)?;
    let lock = private_file(&dir.join("instance.lock"), false)?;
    lock.try_lock()
        .context("Another service owns this data directory; use qinaegis open or status")?;
    let mut last_error = None;
    let ports: Vec<u16> = options
        .port
        .map(|p| vec![p])
        .unwrap_or_else(|| (DEFAULT_PORT..DEFAULT_PORT + 10).collect());
    let mut listener = None;
    for port in ports {
        match tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await {
            Ok(bound) => {
                listener = Some(bound);
                break;
            }
            Err(error) => last_error = Some(error),
        }
    }
    let listener =
        listener.ok_or_else(|| anyhow::anyhow!("Cannot bind local service: {:?}", last_error))?;
    let origin = format!("http://{}", listener.local_addr()?);
    let instance = Instance {
        origin: origin.clone(),
        instance_id: server::random_token()?,
        owner_secret: server::random_token()?,
        pid: std::process::id(),
        version: crate::app::APP_VERSION.into(),
    };
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let state = ServerState::new(
        AppState::new(),
        origin.clone(),
        instance.instance_id.clone(),
        instance.owner_secret.clone(),
        shutdown_tx,
    );
    let router = server::router(state.clone());
    save_instance(&dir.join("instance.json"), &instance)?;
    println!(
        "QinAegis {} serving at {origin}; use qinaegis open to authorize a browser",
        instance.version
    );
    if !options.no_open {
        let token = state.launch_token()?;
        let url = format!("{origin}/#token={token}");
        if let Err(error) = open_browser(&url) {
            eprintln!("Browser did not open: {error}. Use qinaegis open after readiness.");
        }
    }
    let result = axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            loop {
                tokio::select! {
                    result = tokio::signal::ctrl_c() => {
                        if result.is_err() { break; }
                        match state.begin_shutdown() {
                            Ok(()) => break,
                            Err(message) => eprintln!("Cannot stop service: {message}"),
                        }
                    },
                    _ = shutdown_rx.changed() => break,
                }
            }
        })
        .await;
    let _ = std::fs::remove_file(dir.join("instance.json"));
    drop(lock);
    result?;
    Ok(())
}

pub async fn run(args: Vec<String>) -> Result<()> {
    let options = Options::parse(args)?;
    match options.command.as_str() {
        "version"=>println!("QinAegis {} (API v2)",crate::app::APP_VERSION),
        "help"=>println!("qinaegis [start|serve|open|status|stop|version] [--port PORT] [--no-open]\nDefault: start a local background service and open your browser.\nserve: foreground; Ctrl+C stops service. close tabs without stopping background jobs.\nopen: authorize a fresh browser session. stop: refuses while tasks are running."),
        _=>{
            let dir=state_dir()?;
            match options.command.as_str() {
                "start"=>start(&options,&dir).await?,
                "serve"=>serve(&options,&dir).await?,
                "status"=>match read_instance(&dir) {
                    Ok(instance) if healthy(&instance).await=>println!("QinAegis {} running (PID {}) at {}\nState/logs: {}",instance.version,instance.pid,instance.origin,dir.display()),
                    _=>bail!("QinAegis is not running"),
                },
                "open"=>{ let instance=read_instance(&dir).context("Service is not running; use qinaegis start")?; ensure!(healthy(&instance).await,"Service is not running; use qinaegis start"); show_browser(&instance).await?; },
                "stop"=>{ let instance=read_instance(&dir).context("Service is not running")?; ensure!(healthy(&instance).await,"Service is not running");
                    let response=client()?.post(format!("{}/api/v2/shutdown",instance.origin)).header("x-qinaegis-owner",&instance.owner_secret).send().await?;
                    if !response.status().is_success() { bail!("Cannot stop service: {}",response.text().await?); }
                    for _ in 0..30 { if !healthy(&instance).await { println!("QinAegis stopped");return Ok(()); } tokio::time::sleep(Duration::from_millis(100)).await; }
                    bail!("Stop requested; service still shutting down");
                },
                _=>unreachable!(),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_command_options_do_not_accept_public_listeners() {
        assert_eq!(Options::parse(vec![]).unwrap().command, "start");
        assert!(Options::parse(vec!["--port".into(), "0".into()]).is_err());
        assert!(Options::parse(vec!["--host".into(), "0.0.0.0".into()]).is_err());
        assert!(Options::parse(vec!["stop".into(), "--port".into(), "99".into()]).is_err());
        assert_eq!(
            Options::parse(vec![
                "serve".into(),
                "--no-open".into(),
                "--port".into(),
                "19000".into()
            ])
            .unwrap(),
            Options {
                command: "serve".into(),
                port: Some(19000),
                no_open: true
            }
        );
    }
    #[test]
    fn tokens_are_independent_and_full_entropy_length() {
        let a = server::random_token().unwrap();
        let b = server::random_token().unwrap();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert!(a.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}
