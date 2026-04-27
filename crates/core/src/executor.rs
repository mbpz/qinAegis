use crate::protocol::{JsonRpcRequest, MidsceneProcess, LlmConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseRef {
    pub id: String,
    pub yaml_script: String,
    pub name: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub case_id: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub screenshot_base64: Option<String>,
    pub error_message: Option<String>,
}

pub struct TestExecutor {
    process: Arc<Mutex<MidsceneProcess>>,
    semaphore: Arc<Semaphore>,
}

impl TestExecutor {
    pub async fn new(max_concurrency: usize, llm_config: Option<LlmConfig>) -> anyhow::Result<Self> {
        let process = MidsceneProcess::spawn(llm_config).await?;
        let semaphore = Arc::new(Semaphore::new(max_concurrency));
        Ok(Self {
            process: Arc::new(Mutex::new(process)),
            semaphore,
        })
    }

    pub async fn run_case(&self, case: &TestCaseRef) -> anyhow::Result<TestResult> {
        let _permit = self.semaphore.acquire().await?;

        let req = JsonRpcRequest::RunYaml {
            yaml_script: case.yaml_script.clone(),
            case_id: case.id.clone(),
        };

        let mut process = self.process.lock().await;
        let resp = process.call(req).await?;

        if resp.ok {
            let result: TestResult = serde_json::from_value(resp.data.unwrap_or_default())?;
            Ok(result)
        } else {
            Ok(TestResult {
                case_id: case.id.clone(),
                passed: false,
                duration_ms: 0,
                screenshot_base64: None,
                error_message: resp.error,
            })
        }
    }

    pub async fn run_parallel(&self, cases: Vec<TestCaseRef>) -> anyhow::Result<Vec<TestResult>> {
        let mut handles: Vec<tokio::task::JoinHandle<anyhow::Result<TestResult>>> = Vec::new();
        let semaphore = self.semaphore.clone();
        let process = self.process.clone();

        for case in cases {
            let case = case.clone();
            let sem = semaphore.clone();
            let proc = process.clone();
            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let req = JsonRpcRequest::RunYaml {
                    yaml_script: case.yaml_script.clone(),
                    case_id: case.id.clone(),
                };
                let resp = {
                    let mut p = proc.lock().await;
                    p.call(req).await
                };
                match resp {
                    Ok(resp) if resp.ok => {
                        Ok(serde_json::from_value(resp.data.unwrap_or_default()).unwrap_or_else(|_| TestResult {
                            case_id: case.id.clone(),
                            passed: false,
                            duration_ms: 0,
                            screenshot_base64: None,
                            error_message: Some("parse error".to_string()),
                        }))
                    }
                    Ok(resp) => Ok(TestResult {
                        case_id: case.id.clone(),
                        passed: false,
                        duration_ms: 0,
                        screenshot_base64: None,
                        error_message: resp.error,
                    }),
                    Err(e) => Ok(TestResult {
                        case_id: case.id.clone(),
                        passed: false,
                        duration_ms: 0,
                        screenshot_base64: None,
                        error_message: Some(e.to_string()),
                    }),
                }
            });
            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await??);
        }
        Ok(results)
    }

    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let mut process = self.process.lock().await;
        process.call(JsonRpcRequest::Shutdown).await?;
        Ok(())
    }
}

impl Clone for TestExecutor {
    fn clone(&self) -> Self {
        Self {
            process: self.process.clone(),
            semaphore: self.semaphore.clone(),
        }
    }
}
