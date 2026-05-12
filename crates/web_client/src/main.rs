use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::{Arc, Mutex};

mod assets;

fn main() -> Result<()> {
    let event_loop = tao::event_loop::EventLoop::new();

    let window = tao::window::WindowBuilder::new()
        .with_title("QinAegis")
        .with_inner_size(tao::dpi::LogicalSize::new(1200.0, 800.0))
        .build(&event_loop)?;

    let _app = Arc::new(Mutex::new(AppState::default()));

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "ios", target_os = "android"))]
    {
        let webview = wry::WebViewBuilder::new()
            .with_custom_protocol("app".into(), move |_id, request| {
                let path = request.uri().path().trim_start_matches('/');
                let (contents, mime) = assets::getAsset(path);
                wry::http::Response::builder()
                    .header("Content-Type", mime)
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Cow::Owned(contents.to_vec()))
                    .unwrap()
            })
            .with_url("app://localhost/index.html")
            .build(&window)?;

        let init_script = r#"
            window.rpc = function(method, params) {
                return new Promise(function(resolve) {
                    var id = Date.now();
                    var callback = {resolve: resolve};
                    window.__rpcCallbacks = window.__rpcCallbacks || {};
                    window.__rpcCallbacks[id] = callback;
                    window.eval('window.__handleRpc("'"'"'{"method":"'"'"' + method + "'"'"',"params":"'"'"' + JSON.stringify(params) + "'"'"',"id":"'"'"' + id + '}"'"'"' + ')'"'"');
                    setTimeout(function() {
                        if (window.__rpcCallbacks && window.__rpcCallbacks[id]) {
                            window.__rpcCallbacks[id].resolve({error: "timeout"});
                            delete window.__rpcCallbacks[id];
                        }
                    }, 30000);
                });
            };
            window.__handleRpc = function(data) {
                var method = data.method;
                var params = typeof data.params === 'string' ? JSON.parse(data.params) : data.params;
                var id = data.id;
                var result;
                if (method === 'getState') {
                    result = '{"ok":true}';
                } else if (method === 'setConfig') {
                    result = '{"ok":true}';
                } else if (method === 'runExplore') {
                    result = '{"ok":true,"jobId":"explore-1"}';
                } else if (method === 'runGenerate') {
                    result = '{"ok":true,"jobId":"generate-1"}';
                } else if (method === 'runTests') {
                    result = '{"ok":true,"jobId":"run-1"}';
                } else if (method === 'getOutput') {
                    result = '{"output":""}';
                } else if (method === 'getProjects') {
                    result = '["demo-project"]';
                } else {
                    result = '{"error":"unknown method"}';
                }
                if (window.__rpcCallbacks && window.__rpcCallbacks[id]) {
                    window.__rpcCallbacks[id].resolve(JSON.parse(result));
                    delete window.__rpcCallbacks[id];
                }
            };
            window.getState = function() { return window.rpc('getState', {}); };
            window.setConfig = function(c) { return window.rpc('setConfig', {config: c}); };
            window.runExplore = function(url, depth) { return window.rpc('runExplore', {url: url, depth: depth}); };
            window.runGenerate = function(req, spec) { return window.rpc('runGenerate', {requirement: req, spec: spec}); };
            window.runTests = function(project, type) { return window.rpc('runTests', {project: project, type: type}); };
            window.getOutput = function() { return window.rpc('getOutput', {}); };
            window.getProjects = function() { return window.rpc('getProjects', {}); };
            console.log('RPC bridge ready');
        "#;

        webview.evaluate_script(init_script)?;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = tao::event_loop::ControlFlow::Wait;

            if let tao::event::Event::WindowEvent {
                event: tao::event::WindowEvent::CloseRequested,
                ..
            } = event {
                *control_flow = tao::event_loop::ControlFlow::Exit;
            }
        })
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios", target_os = "android")))]
    {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().unwrap();
        let _webview = wry::WebViewBuilder::new()
            .with_custom_protocol("app".into(), move |_id, request| {
                let path = request.uri().path().trim_start_matches('/');
                let (contents, mime) = assets::getAsset(path);
                wry::http::Response::builder()
                    .header("Content-Type", mime)
                    .body(Cow::Owned(contents.to_vec()))
                    .unwrap()
            })
            .with_url("app://localhost/index.html")
            .build_gtk(vbox)?;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = tao::event_loop::ControlFlow::Wait;
            if let tao::event::Event::WindowEvent {
                event: tao::event::WindowEvent::CloseRequested,
                ..
            } = event {
                *control_flow = tao::event_loop::ControlFlow::Exit;
            }
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AppState {
    pub config: serde_json::Value,
    pub output: String,
    pub current_view: String,
}

impl AppState {
    pub fn get_state(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn set_config(&mut self, config: serde_json::Value) -> String {
        self.config = config;
        r#"{"ok":true}"#.to_string()
    }

    pub fn run_explore(&mut self, url: String, depth: u32) -> String {
        self.output.push_str(&format!("[Explore] url={}, depth={}\n", url, depth));
        r#"{"ok":true,"jobId":"explore-1"}"#.to_string()
    }

    pub fn run_generate(&mut self, requirement: String, spec: Option<String>) -> String {
        self.output.push_str(&format!("[Generate] requirement={}, spec={:?}\n", requirement, spec));
        r#"{"ok":true,"jobId":"generate-1"}"#.to_string()
    }

    pub fn run_tests(&mut self, project: String, test_type: String) -> String {
        self.output.push_str(&format!("[Run] project={}, type={}\n", project, test_type));
        r#"{"ok":true,"jobId":"run-1"}"#.to_string()
    }

    pub fn get_output(&self) -> String {
        self.output.clone()
    }

    pub fn get_projects(&self) -> String {
        r#"["demo-project"]"#.to_string()
    }
}
