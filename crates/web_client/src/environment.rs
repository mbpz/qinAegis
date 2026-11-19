//! Fixed runtime checks and browser installation; never executes browser-supplied commands.
use anyhow::{ensure, Context, Result};
use qin_aegis_core::runtime::{RuntimeCommand, RuntimeResolver};
use serde_json::{json, Value};
use std::process::Stdio;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};

pub async fn cancelled(cancel: Arc<AtomicBool>) {
    while !cancel.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn command(runtime: &RuntimeCommand) -> Result<Command> {
    let mut cmd = Command::new(&runtime.node);
    cmd.current_dir(&runtime.directory)
        .kill_on_drop(true)
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH")
        .env("QINAEGIS_CHECK_ROOT", &runtime.directory);
    if let Some(browsers) = runtime.browser_directory()? {
        cmd.env("PLAYWRIGHT_BROWSERS_PATH", browsers);
    }
    Ok(cmd)
}

const MAX_PROBE_OUTPUT: usize = 16 * 1024;

struct ManagedChild {
    child: Child,
    tree: qin_aegis_core::process_owner::ProcessTree,
    stopped: bool,
}

impl ManagedChild {
    fn stop_now(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        self.tree.terminate_now(&mut self.child);
    }

    async fn stop(&mut self) -> Result<()> {
        self.stop_now();
        tokio::time::timeout(Duration::from_secs(5), async {
            self.child
                .wait()
                .await
                .map_err(|_| anyhow::anyhow!("Environment process cleanup failed"))?;
            while !self
                .tree
                .cleanup_complete()
                .map_err(|_| anyhow::anyhow!("Environment process tree cleanup failed"))?
            {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Ok(())
        })
        .await
        .map_err(|_| anyhow::anyhow!("Environment process tree cleanup timed out"))?
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        if self.child.id().is_none() {
            return;
        }
        self.stop_now();
        // A dropped future cannot await. Give the killed direct child a bounded
        // opportunity to be reaped even when its Tokio runtime is shutting down.
        let deadline = std::time::Instant::now() + Duration::from_millis(100);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Ok(None) => break,
            }
        }
    }
}

async fn run_managed(
    cmd: &mut Command,
    cancel: Arc<AtomicBool>,
    budget: Duration,
    capture: bool,
) -> Result<Vec<u8>> {
    run_managed_reported(cmd, cancel, budget, capture, None).await
}

async fn run_managed_reported(
    cmd: &mut Command,
    cancel: Arc<AtomicBool>,
    budget: Duration,
    capture: bool,
    progress: Option<crate::app::JobReporter>,
) -> Result<Vec<u8>> {
    ensure!(
        !cancel.load(Ordering::Acquire),
        "Environment operation cancelled"
    );
    cmd.stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    cmd.stdout(if capture || progress.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let (mut child, tree) = qin_aegis_core::process_owner::ProcessTree::spawn(cmd)
        .map_err(|_| anyhow::anyhow!("Environment process could not start with owned tree"))?;
    let stdout = child.stdout.take();
    let mut owned = ManagedChild {
        child,
        tree,
        stopped: false,
    };
    let operation = async {
        let mut bytes = Vec::new();
        if let Some(mut stdout) = stdout {
            if let Some(report) = progress {
                let mut parser = InstallProgress::default();
                let mut buffer = [0u8; 1024];
                let mut total = 0usize;
                loop {
                    let count = stdout
                        .read(&mut buffer)
                        .await
                        .map_err(|_| anyhow::anyhow!("Environment output unavailable"))?;
                    if count == 0 {
                        break;
                    }
                    total += count;
                    ensure!(
                        total <= 4 * 1024 * 1024,
                        "Browser installation output exceeds limit"
                    );
                    parser.feed(&buffer[..count], report.as_ref());
                }
            } else {
                stdout
                    .take((MAX_PROBE_OUTPUT + 1) as u64)
                    .read_to_end(&mut bytes)
                    .await
                    .map_err(|_| anyhow::anyhow!("Environment output unavailable"))?;
                ensure!(
                    bytes.len() <= MAX_PROBE_OUTPUT,
                    "Environment output exceeds limit"
                );
            }
        }
        let status = owned
            .child
            .wait()
            .await
            .map_err(|_| anyhow::anyhow!("Environment process failed"))?;
        ensure!(
            status.success(),
            "Environment process failed; check installation, network and OS dependencies"
        );
        Ok(bytes)
    };
    let result = tokio::select! {
        biased;
        _ = cancelled(cancel) => Err(anyhow::anyhow!("Environment operation cancelled")),
        output = tokio::time::timeout(budget, operation) => output.map_err(|_| anyhow::anyhow!("Environment operation timed out")).and_then(|value|value),
    };
    // A successful direct child can still leave descendants. Confirm tree
    // cleanup on both paths before any runtime directory may be switched.
    match (result, owned.stop().await) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(anyhow::anyhow!("{error}; cleanup failed: {cleanup}")),
    }
}

/// Recognize only the locked Playwright non-TTY progress format. Never echo
/// installer text: download URLs and paths may carry private information.
#[derive(Default)]
struct InstallProgress {
    line: Vec<u8>,
    oversized: bool,
    percentage: Option<u8>,
}

impl InstallProgress {
    fn feed(&mut self, bytes: &[u8], report: &dyn Fn(&str)) {
        for byte in bytes {
            if matches!(byte, b'\r' | b'\n') {
                if !self.oversized {
                    self.emit(report);
                }
                self.line.clear();
                self.oversized = false;
            } else if self.line.len() < 4096 && !self.oversized {
                self.line.push(*byte);
            } else {
                self.line.clear();
                self.oversized = true;
            }
        }
    }

    fn emit(&mut self, report: &dyn Fn(&str)) {
        let Ok(line) = std::str::from_utf8(&self.line) else {
            return;
        };
        if line.starts_with("Downloading ") {
            self.percentage = None;
            report("Downloading a matching Playwright browser component.\n");
        } else if let Some(bar) = line.strip_prefix('|').and_then(|s| s.split_once('|')) {
            if !bar.0.chars().all(|c| c == ' ' || c == '\u{25a0}') {
                return;
            }
            if let Some((number, _)) = bar.1.trim_start().split_once("% of ") {
                if let Ok(value) = number.parse::<u8>() {
                    if value <= 100 && self.percentage.is_none_or(|last| value > last) {
                        self.percentage = Some(value);
                        report(&format!("Current browser component download: {value}%.\n"));
                    }
                }
            }
        }
    }
}

pub(crate) fn healthcheck_at(root: &std::path::Path, cancel: Arc<AtomicBool>) -> Result<()> {
    let runtime = RuntimeResolver::installed(root)?;
    let mut cmd = command(&runtime)?;
    cmd.args(["-e", r#"
const fs=require('node:fs'),path=require('node:path'),{pathToFileURL}=require('node:url');
(async()=>{
 const root=process.env.QINAEGIS_CHECK_ROOT;
 if(process.versions.node.split('.')[0]!=='24')throw Error('node version');
 const manifest=JSON.parse(fs.readFileSync(path.join(root,'runtime.json')));
 const provenance=JSON.parse(fs.readFileSync(path.join(root,'provenance.json')));
 for(const name of ['playwright','@midscene/web','allure']){
  const installed=JSON.parse(fs.readFileSync(path.join(root,'node_modules',name,'package.json'))).version;
  if(typeof installed!=='string'||installed!==provenance.packages?.[name])throw Error('package version');
 }
 await import(pathToFileURL(path.join(root,manifest.entry)).href);
 process.stdout.write(JSON.stringify({healthy:true}));
})().catch(()=>process.exit(1));
"#]);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let bytes = rt.block_on(run_managed(&mut cmd, cancel, Duration::from_secs(30), true))?;
    ensure!(
        serde_json::from_slice::<Value>(&bytes)?["healthy"] == true,
        "Runtime health check failed; previous runtime retained"
    );
    Ok(())
}

// Version facts come from installed packages and a real headless browser launch.
const PROBE: &str = r#"
const fs = require('node:fs');
const path = require('node:path');
const root = process.env.QINAEGIS_CHECK_ROOT;
const result = { node: process.versions.node, packages: {}, browser: { installed: false, launched: false } };
(async () => {
  for (const name of ['playwright', '@midscene/web', 'allure']) {
    try { result.packages[name] = JSON.parse(fs.readFileSync(path.join(root, 'node_modules', name, 'package.json'), 'utf8')).version; }
    catch { result.packages[name] = null; }
  }
  try {
    const { chromium } = require(path.join(root, 'node_modules', 'playwright'));
    result.browser.installed = fs.existsSync(chromium.executablePath());
    const browser = await chromium.launch({ headless: true, timeout: 10000 });
    result.browser.launched = true;
    result.browser.version = browser.version();
    await browser.close();
  } catch { result.browser.error = 'Chromium could not launch; install its matching browser or required OS libraries.'; }
  console.log(JSON.stringify(result));
})().catch(() => process.exit(1));
"#;

pub fn diagnose(cancel: Arc<AtomicBool>) -> Result<Value> {
    let runtime = match RuntimeResolver::resolve() {
        Ok(runtime) => runtime,
        Err(_) => {
            return Ok(
                json!({"ready":false,"runtime_available":false,"repair":"Install a verified standalone runtime. Asset management remains available."}),
            )
        }
    };
    let mut cmd = command(&runtime)?;
    cmd.args(["-e", PROBE]);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let output = rt.block_on(run_managed(&mut cmd, cancel, Duration::from_secs(20), true))?;
    let mut value: Value =
        serde_json::from_slice(&output).context("Invalid runtime check response")?;
    let node_ok = value["node"]
        .as_str()
        .and_then(|s| s.split('.').next())
        .and_then(|s| s.parse::<u32>().ok())
        .is_some_and(|n| n == 24);
    value["node_supported"] = json!(node_ok);
    value["runtime_available"] = json!(true);
    value["engine_version"] = json!(runtime.version);
    value["mode"] = json!(if runtime.version == "developer" {
        "developer"
    } else {
        "installed"
    });
    value["ready"] = json!(
        node_ok
            && value["browser"]["launched"] == true
            && ["playwright", "@midscene/web", "allure"]
                .iter()
                .all(|name| value["packages"][*name].is_string())
    );
    Ok(value)
}

pub fn install_browser(cancel: Arc<AtomicBool>, report: crate::app::JobReporter) -> Result<Value> {
    report("Checking the selected runtime and matching Chromium installer.\n");
    let runtime = RuntimeResolver::resolve()?;
    let cli = runtime
        .directory
        .join("node_modules/playwright/cli.js")
        .canonicalize()
        .context("Playwright CLI missing")?;
    ensure!(
        cli.starts_with(&runtime.directory),
        "Playwright CLI escapes runtime"
    );
    let mut cmd = command(&runtime)?;
    cmd.arg(cli).args(["install", "chromium"]);
    // Only finite progress facts are emitted; raw child text is never persisted.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    report("Installing Chromium in the selected runtime cache; cancellation waits for owned processes to exit.\n");
    let result = rt.block_on(run_managed_reported(
        &mut cmd,
        cancel.clone(),
        Duration::from_secs(600),
        false,
        Some(report.clone()),
    ));
    if let Err(error) = result {
        report("Browser installation ended; owned process cleanup has finished or its failure is recorded. Check the task error, then retry with download approval.\n");
        return Err(error);
    }
    report(
        "Installer exited. Verifying package versions and launching an actual headless Chromium.\n",
    );
    let facts = diagnose(cancel)?;
    report(if facts["ready"] == true {
        "Chromium launch verification passed.\n"
    } else {
        "Chromium needs attention. Inspect the launch diagnostic and required operating system libraries before retrying.\n"
    });
    Ok(facts)
}

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use super::*;

    struct TestDir(std::path::PathBuf);
    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "qinaegis-environment-{}",
                crate::server::random_token().unwrap()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fixture(marker: &std::path::Path) -> Command {
        let script = r#"
import os,subprocess,sys,time
child=subprocess.Popen([sys.executable,'-u','-c','import socket,time; s=socket.socket(); s.bind(("127.0.0.1",0)); s.listen(); print(s.getsockname()[1],flush=True); time.sleep(120)'],stdout=subprocess.PIPE,text=True)
port=child.stdout.readline().strip()
with open(sys.argv[1]+'.pid','w') as output: output.write(str(os.getpid()))
with open(sys.argv[1],'w') as output: output.write(port)
print('Downloading secret-url-token',flush=True)
print('|    | 10% of secret-url-token',flush=True)
time.sleep(120)
"#;
        let mut cmd = Command::new(if cfg!(windows) { "python" } else { "python3" });
        cmd.args(["-u", "-c", script]).arg(marker);
        cmd
    }

    async fn fixture_port(marker: &std::path::Path) -> u16 {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(value) = std::fs::read_to_string(marker) {
                    if let Ok(port) = value.parse::<u16>() {
                        break port;
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("fixture failed to start")
    }

    async fn released(port: u16) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("managed grandchild retained socket")
    }

    #[tokio::test]
    async fn cancellation_terminates_owned_group() {
        let temporary = TestDir::new();
        let marker = temporary.path().join("port");
        let mut cmd = fixture(&marker);
        let cancel = Arc::new(AtomicBool::new(false));
        let token = cancel.clone();
        let task = tokio::spawn(async move {
            run_managed(&mut cmd, token, Duration::from_secs(30), false).await
        });
        let port = fixture_port(&marker).await;
        assert!(std::net::TcpListener::bind(("127.0.0.1", port)).is_err());
        cancel.store(true, Ordering::Release);
        assert!(task
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("cancelled"));
        released(port).await;
    }

    #[tokio::test]
    async fn timeout_terminates_owned_group() {
        let temporary = TestDir::new();
        let marker = temporary.path().join("port");
        let mut cmd = fixture(&marker);
        let task = tokio::spawn(async move {
            run_managed(
                &mut cmd,
                Arc::new(AtomicBool::new(false)),
                Duration::from_secs(2),
                false,
            )
            .await
        });
        let port = fixture_port(&marker).await;
        assert!(task
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("timed out"));
        released(port).await;
    }

    #[tokio::test]
    async fn dropping_future_terminates_owned_group() {
        let temporary = TestDir::new();
        let marker = temporary.path().join("port");
        let mut cmd = fixture(&marker);
        let task = tokio::spawn(async move {
            run_managed(
                &mut cmd,
                Arc::new(AtomicBool::new(false)),
                Duration::from_secs(30),
                true,
            )
            .await
        });
        let port = fixture_port(&marker).await;
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        #[cfg(unix)]
        {
            let pid: i32 = std::fs::read_to_string(marker.with_extension("pid"))
                .unwrap()
                .parse()
                .unwrap();
            unsafe extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            assert_eq!(unsafe { kill(pid, 0) }, -1, "direct child was not reaped");
        }
        released(port).await;
    }

    #[tokio::test]
    async fn output_limit_stops_process_without_echoing_untrusted_content() {
        let mut cmd = Command::new(if cfg!(windows) { "python" } else { "python3" });
        cmd.args(["-u", "-c", "import sys,time; sys.stdout.write('secret-url-token'*2000); sys.stdout.flush(); time.sleep(120)"]);
        let error = run_managed(
            &mut cmd,
            Arc::new(AtomicBool::new(false)),
            Duration::from_secs(5),
            true,
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(error.contains("exceeds limit"));
        assert!(!error.contains("secret-url-token"));
    }

    #[tokio::test]
    async fn pre_cancelled_operation_does_not_start() {
        let mut cmd = Command::new("must-not-run-invalid-command");
        let error = run_managed(
            &mut cmd,
            Arc::new(AtomicBool::new(true)),
            Duration::from_secs(1),
            true,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
    }

    #[test]
    fn installer_progress_is_bounded_chunk_safe_and_never_echoes_private_text() {
        let messages = std::sync::Mutex::new(String::new());
        let report = |value: &str| messages.lock().unwrap().push_str(value);
        let mut parser = InstallProgress::default();
        for chunk in b"Downloading Chromium from https://private.example/key\n|    | 10% of 2 MiB\n|    | 10% of secret\n|    | 5% of secret\n|    | 110% of secret\n".chunks(3) {
            parser.feed(chunk, &report);
        }
        parser.feed(&vec![b'x'; 4097], &report);
        parser.feed(b"Downloading secret\n", &report);
        parser.feed(
            b"Downloading another private component\n|    | 0% of secret\n|    | 100% of secret\n",
            &report,
        );
        let result = messages.lock().unwrap();
        assert_eq!(result.matches("Downloading a matching").count(), 2);
        assert_eq!(result.matches("10%.").count(), 1);
        assert!(result.contains("0%.\n") && result.contains("100%.\n"));
        assert!(
            !result.contains("private") && !result.contains("secret") && !result.contains("110%")
        );
        assert!(parser.line.len() <= 4096);
    }

    #[tokio::test]
    async fn installer_reports_live_progress_then_cleans_up_on_cancel_and_allows_retry() {
        let temporary = TestDir::new();
        let marker = temporary.path().join("port");
        let mut cmd = fixture(&marker);
        // The fixture's child still owns a real socket. Its parent writes a
        // locked-format progress line before sleeping, so progress must arrive
        // before cancellation, rather than being buffered until process exit.
        cmd.env("PYTHONINSPECT", "");
        let messages = Arc::new(std::sync::Mutex::new(String::new()));
        let sink = messages.clone();
        let report: crate::app::JobReporter = Arc::new(move |s| sink.lock().unwrap().push_str(s));
        let cancel = Arc::new(AtomicBool::new(false));
        let token = cancel.clone();
        let task = tokio::spawn(async move {
            run_managed_reported(
                &mut cmd,
                token,
                Duration::from_secs(30),
                false,
                Some(report),
            )
            .await
        });
        let port = fixture_port(&marker).await;
        tokio::time::timeout(Duration::from_secs(3), async {
            while !messages.lock().unwrap().contains("10%.") {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("progress was not visible while the installer ran");
        cancel.store(true, Ordering::Release);
        assert!(task
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("cancelled"));
        released(port).await;
        assert!(!messages.lock().unwrap().contains("secret"));
        let mut retry = Command::new(if cfg!(windows) { "python" } else { "python3" });
        retry.args([
            "-c",
            "print('Downloading secret'); print('|    | 100% of secret')",
        ]);
        let sink = messages.clone();
        let report: crate::app::JobReporter = Arc::new(move |s| sink.lock().unwrap().push_str(s));
        run_managed_reported(
            &mut retry,
            Arc::new(AtomicBool::new(false)),
            Duration::from_secs(5),
            false,
            Some(report),
        )
        .await
        .unwrap();
        assert!(messages.lock().unwrap().contains("100%."));
    }
}
