// Copyright (c) 2026 QinAegis Team
// SPDX-License-Identifier: MIT

use crate::protocol::{JsonRpcRequest, MidsceneProcess, LlmConfig, SandboxConfig};
use crate::healer::Healer;
use crate::llm::ArcLlmClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseRef {
    pub id: String,
    pub yaml_script: String,
    pub name: String,
    pub priority: String,
    pub target_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub case_id: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub screenshot_base64: Option<String>,
    pub error_message: Option<String>,
    /// Number of self-heal retries attempted (0 = no healing attempted)
    #[serde(default)]
    pub retry_count: u8,
    /// Healed YAML script (original is preserved in TestCaseRef)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healed_yaml_script: Option<String>,
}

pub struct TestExecutor {
    process: Arc<Mutex<MidsceneProcess>>,
    semaphore: Arc<Semaphore>,
    healer: Option<Healer>,
    max_heal_retries: u8,
}

impl TestExecutor {
    pub async fn new(max_concurrency: usize, llm_config: Option<LlmConfig>, sandbox_config: Option<SandboxConfig>) -> anyhow::Result<Self> {
        let process = MidsceneProcess::spawn(llm_config.clone(), sandbox_config).await?;
        let semaphore = Arc::new(Semaphore::new(max_concurrency));
        let healer = llm_config.as_ref().and_then(|cfg| {
            if cfg.api_key.is_empty() {
                None
            } else {
                Some(Healer::new(ArcLlmClient::new(crate::MiniMaxClient::new(
                    cfg.base_url.clone(),
                    cfg.api_key.clone(),
                    cfg.model.clone(),
                ))))
            }
        });
        Ok(Self {
            process: Arc::new(Mutex::new(process)),
            semaphore,
            healer,
            max_heal_retries: 1,
        })
    }

    /// Run a single test case. On failure, attempt self-heal once.
    pub async fn run_case(&self, case: &TestCaseRef) -> anyhow::Result<TestResult> {
        let _permit = self.semaphore.acquire().await?;

        let req = JsonRpcRequest::RunYaml {
            yaml_script: case.yaml_script.clone(),
            case_id: case.id.clone(),
            target_url: case.target_url.clone(),
        };

        let process = self.process.lock().await;
        let resp = process.call(req).await?;

        if resp.ok {
            let mut result: TestResult = serde_json::from_value(resp.data.unwrap_or_default())?;
            result.retry_count = 0;
            Ok(result)
        } else {
            let error_msg = resp.error.clone().unwrap_or_else(|| "unknown error".to_string());
            drop(process);

            // Attempt self-heal if healer is available
            if let Some(ref healer) = self.healer {
                if self.max_heal_retries > 0 {
                    tracing::info!("Self-healing attempt for case {}: {}", case.id, error_msg);
                    if let Some(healed_yaml) = healer.heal(&case.yaml_script, "aiAssert", &error_msg).await {
                        // Retry with healed script
                        let retry_result = self.run_case_with_yaml(case, &healed_yaml).await;
                        if let Ok(mut result) = retry_result {
                            result.retry_count = 1;
                            result.healed_yaml_script = Some(healed_yaml);
                            if result.passed {
                                tracing::info!("Self-heal succeeded for case {}", case.id);
                            } else {
                                tracing::warn!("Self-heal failed for case {}, returning original failure", case.id);
                            }
                            return Ok(result);
                        }
                    }
                }
            }

            Ok(TestResult {
                case_id: case.id.clone(),
                passed: false,
                duration_ms: 0,
                screenshot_base64: None,
                error_message: Some(error_msg),
                retry_count: 0,
                healed_yaml_script: None,
            })
        }
    }

    async fn run_case_with_yaml(&self, case: &TestCaseRef, yaml_script: &str) -> anyhow::Result<TestResult> {
        let _permit = self.semaphore.acquire().await?;

        let req = JsonRpcRequest::RunYaml {
            yaml_script: yaml_script.to_string(),
            case_id: case.id.clone(),
            target_url: case.target_url.clone(),
        };

        let process = self.process.lock().await;
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
                retry_count: 1,
                healed_yaml_script: Some(yaml_script.to_string()),
            })
        }
    }

    pub async fn run_parallel(&self, cases: Vec<TestCaseRef>) -> anyhow::Result<Vec<TestResult>> {
        let mut handles: Vec<tokio::task::JoinHandle<anyhow::Result<TestResult>>> = Vec::new();
        let semaphore = self.semaphore.clone();
        let process = self.process.clone();
        let healer = self.healer.clone();
        let max_heal_retries = self.max_heal_retries;

        for case in cases {
            let case = case.clone();
            let sem = semaphore.clone();
            let proc = process.clone();
            let heal = healer.clone();
            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let req = JsonRpcRequest::RunYaml {
                    yaml_script: case.yaml_script.clone(),
                    case_id: case.id.clone(),
                    target_url: case.target_url.clone(),
                };
                let resp = {
                    let p = proc.lock().await;
                    p.call(req).await
                };

                match resp {
                    Ok(resp) if resp.ok => {
                        let mut result: TestResult = serde_json::from_value(resp.data.unwrap_or_default()).unwrap_or_else(|_| TestResult {
                            case_id: case.id.clone(),
                            passed: false,
                            duration_ms: 0,
                            screenshot_base64: None,
                            error_message: Some("parse error".to_string()),
                            retry_count: 0,
                            healed_yaml_script: None,
                        });
                        result.retry_count = 0;
                        Ok(result)
                    }
                    Ok(resp) => {
                        let error_msg = resp.error.clone().unwrap_or_else(|| "unknown error".to_string());

                        if let Some(ref h) = heal {
                            if max_heal_retries > 0 {
                                tracing::info!("Self-heal attempt for case {}: {}", case.id, error_msg);
                                if let Some(healed_yaml) = h.heal(&case.yaml_script, "aiAssert", &error_msg).await {
                                    let heal_req = JsonRpcRequest::RunYaml {
                                        yaml_script: healed_yaml.clone(),
                                        case_id: case.id.clone(),
                                        target_url: case.target_url.clone(),
                                    };
                                    let heal_resp = {
                                        let p = proc.lock().await;
                                        p.call(heal_req).await
                                    };
                                    if let Ok(heal_resp) = heal_resp {
                                        if heal_resp.ok {
                                            let mut result: TestResult = serde_json::from_value(heal_resp.data.unwrap_or_default()).unwrap_or_else(|_| TestResult {
                                                case_id: case.id.clone(),
                                                passed: false,
                                                duration_ms: 0,
                                                screenshot_base64: None,
                                                error_message: Some("parse error".to_string()),
                                                retry_count: 0,
                                                healed_yaml_script: None,
                                            });
                                            result.retry_count = 1;
                                            result.healed_yaml_script = Some(healed_yaml);
                                            return Ok(result);
                                        }
                                    }
                                }
                            }
                        }

                        Ok(TestResult {
                            case_id: case.id.clone(),
                            passed: false,
                            duration_ms: 0,
                            screenshot_base64: None,
                            error_message: Some(error_msg),
                            retry_count: 0,
                            healed_yaml_script: None,
                        })
                    }
                    Err(e) => Ok(TestResult {
                        case_id: case.id.clone(),
                        passed: false,
                        duration_ms: 0,
                        screenshot_base64: None,
                        error_message: Some(e.to_string()),
                        retry_count: 0,
                        healed_yaml_script: None,
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
        let process = self.process.lock().await;
        process.call(JsonRpcRequest::Shutdown).await?;
        Ok(())
    }
}

impl Clone for TestExecutor {
    fn clone(&self) -> Self {
        Self {
            process: self.process.clone(),
            semaphore: self.semaphore.clone(),
            healer: self.healer.clone(),
            max_heal_retries: self.max_heal_retries,
        }
    }
}

