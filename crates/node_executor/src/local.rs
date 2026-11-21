use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex},
    time::Duration,
};

use anyhow::Context;
use async_trait::async_trait;
use common::{
    knobs::{NODE_EXECUTOR_IDLE_TIMEOUT, NODE_EXECUTOR_MAX_LIFETIME},
    log_lines::LogLine,
};
use errors::ErrorMetadata;
use futures::{
    select_biased,
    FutureExt,
};
use futures_async_stream::try_stream;
use isolate::bundled_js::node_executor_file;
use rand::Rng;
use reqwest::Client;
use serde_json::Value as JsonValue;
use tempfile::TempDir;
use tokio::{
    time::Instant,
    process::{
        Child,
        Command as TokioCommand,
    },
    sync::{
        mpsc,
        Mutex,
    },
};

use crate::{
    executor::{
        ExecutorRequest,
        InvokeResponse,
        NodeExecutor,
        ARGS_TOO_LARGE_RESPONSE_MESSAGE,
        EXECUTE_TIMEOUT_RESPONSE_JSON,
    },
    handle_node_executor_stream,
    InvokeCompletion,
    NodeExecutorStreamPart,
};

const NVMRC_VERSION: &str = include_str!("../../../.nvmrc");
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_millis(100);
const MAX_HEALTH_CHECK_ATTEMPTS: u32 = 50;

pub struct LocalNodeExecutor {
    inner: Arc<Mutex<Option<Arc<InnerLocalNodeExecutor>>>>,
    config: Arc<LocalNodeExecutorConfig>,
}

struct LocalNodeExecutorConfig {
    node_process_timeout: Duration,
    /// Overrides the initial callback retry backoff in the spawned node
    /// process (read by syscalls.ts at module load). Tests zero this so
    /// callbacks retrying against an unreachable backend settle within test
    /// timeouts.
    callback_initial_backoff: Option<Duration>,
    idle_timeout: Duration,
    max_lifetime: Duration,
}

struct InnerLocalNodeExecutor {
    _source_dir: TempDir,
    client: reqwest::Client,
    _server_handle: Child,
    started_at: Instant,
    last_finished: StdMutex<Instant>,
}

// Drops only after the owned invocation consumes the response, including streaming.
struct ActiveInvocation(Arc<InnerLocalNodeExecutor>);

impl Drop for ActiveInvocation {
    fn drop(&mut self) {
        *self.0.last_finished.lock().unwrap() = Instant::now();
    }
}

impl LocalNodeExecutorConfig {
    fn reaper_period(&self) -> Option<Duration> {
        [self.idle_timeout, self.max_lifetime]
            .into_iter()
            .filter(|timeout| !timeout.is_zero())
            .min()
            .map(|timeout| (timeout / 4).clamp(Duration::from_secs(1), Duration::from_secs(30)))
    }
}

impl InnerLocalNodeExecutor {
    fn should_retire(self: &Arc<Self>, config: &LocalNodeExecutorConfig, now: Instant) -> bool {
        let lifetime_expired = !config.max_lifetime.is_zero()
            && now.duration_since(self.started_at) >= config.max_lifetime;
        let idle_expired = !config.idle_timeout.is_zero()
            && Arc::strong_count(self) == 1
            && now.duration_since(*self.last_finished.lock().unwrap()) >= config.idle_timeout;
        lifetime_expired || idle_expired
    }

    async fn new(config: &LocalNodeExecutorConfig) -> anyhow::Result<Self> {
        tracing::info!("Initializing inner local node executor");
        // Create a single temp directory for both source files and Node.js temp files
        let source_dir = TempDir::new()?;
        let (source, source_map) =
            node_executor_file("local.cjs").expect("local.cjs not generated!");
        let source_map = source_map.context("Missing local.cjs.map")?;
        let source_path = source_dir.path().join("local.cjs");
        let source_map_path = source_dir.path().join("local.cjs.map");
        fs::write(&source_path, source.as_bytes())?;
        fs::write(source_map_path, source_map.as_bytes())?;
        tracing::info!(
            "Using local node executor. Source: {}",
            source_path.to_str().expect("Path is not UTF-8 string?"),
        );

        let socket_path = if cfg!(unix) {
            source_dir.path().join(".executor.sock")
        } else if cfg!(windows) {
            PathBuf::from(format!(
                r"\\.\pipe\cvx-node-executor-{:016x}",
                rand::rng().random::<u64>()
            ))
        } else {
            panic!("not supported");
        };
        let server_handle =
            Self::start_node_with_listener(config, &source_path, &source_dir, &socket_path).await?;
        // Don't keep idle connections in the pool. The Node HTTP server closes
        // idle keep-alive connections after its (default 5s) `keepAliveTimeout`,
        // but hyper's pool would hold one much longer and reuse it right as the
        // server closes it, surfacing as a spurious "connection reset by peer".
        // Opening a fresh connection per request is cheap over a local socket.
        let mut client_builder = Client::builder().pool_max_idle_per_host(0);
        #[cfg(unix)]
        {
            client_builder = client_builder.unix_socket(socket_path);
        }
        #[cfg(windows)]
        {
            client_builder = client_builder.windows_named_pipe(socket_path);
        }
        let client = client_builder.build()?;

        // Wait for the Node process to be ready to handle HTTP requests.
        for _ in 0..MAX_HEALTH_CHECK_ATTEMPTS {
            if Self::check_server_health(&client).await? {
                return Ok(Self {
                    _source_dir: source_dir,
                    client,
                    _server_handle: server_handle,
                    started_at: Instant::now(),
                    last_finished: StdMutex::new(Instant::now()),
                });
            }
            tokio::time::sleep(HEALTH_CHECK_INTERVAL).await;
        }
        anyhow::bail!("Node executor server failed to start and become healthy")
    }

    async fn check_node_version(node_path: &str) -> anyhow::Result<()> {
        let cmd = TokioCommand::new(node_path)
            .arg("--version")
            .output()
            .await?;
        let version = String::from_utf8_lossy(&cmd.stdout);

        if !version.starts_with("v20.")
            && !version.starts_with("v22.")
            && !version.starts_with("v24.")
        {
            anyhow::bail!(ErrorMetadata::bad_request(
                "DeploymentNotConfiguredForNodeActions",
                "Deployment is not configured to deploy \"use node\" actions. \
                 Node.js v20, 22, or 24 is not installed. \
                 Install a supported Node.js version with nvm (https://github.com/nvm-sh/nvm) \
                 to deploy Node.js actions."
            ))
        }
        Ok(())
    }

    async fn check_server_health(client: &Client) -> anyhow::Result<bool> {
        match client
            .get("http://localhost/health".to_string())
            .timeout(Duration::from_secs(1))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => Ok(true),
            _ => Ok(false),
        }
    }

    async fn start_node_with_listener(
        config: &LocalNodeExecutorConfig,
        source_path: &PathBuf,
        temp_dir: &TempDir,
        socket_path: &PathBuf,
    ) -> anyhow::Result<Child> {
        let preferred_node_version = NVMRC_VERSION.trim();

        // Look for node in a few places.
        let possible_path = home::home_dir()
            .unwrap()
            .join(".nvm")
            .join(format!("versions/node/v{preferred_node_version}/bin/node"));
        let node_path = if possible_path.exists() {
            possible_path.to_string_lossy().to_string()
        } else {
            "node".to_string()
        };
        Self::check_node_version(&node_path).await?;

        let mut cmd = TokioCommand::new(node_path);
        cmd.arg(source_path)
            .arg("--ipc-path")
            .arg(socket_path)
            .arg("--tempdir")
            .arg(temp_dir.path())
            .kill_on_drop(true);
        if let Some(backoff) = config.callback_initial_backoff {
            cmd.env(
                "CALLBACK_INITIAL_BACKOFF_MS",
                backoff.as_millis().to_string(),
            );
        }

        let child = cmd.spawn()?;

        Ok(child)
    }
}

impl LocalNodeExecutor {
    pub async fn new(node_process_timeout: Duration) -> anyhow::Result<Self> {
        let executor = Self {
            inner: Arc::new(Mutex::new(None)),
            config: Arc::new(LocalNodeExecutorConfig {
                node_process_timeout,
                callback_initial_backoff: None,
                idle_timeout: *NODE_EXECUTOR_IDLE_TIMEOUT,
                max_lifetime: *NODE_EXECUTOR_MAX_LIFETIME,
            }),
        };

        if let Some(period) = executor.config.reaper_period() {
            // A Weak owner lets the reaper exit after the executor is dropped.
            let weak_inner = Arc::downgrade(&executor.inner);
            let config = executor.config.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(period).await;
                    let Some(inner) = weak_inner.upgrade() else {
                        break;
                    };
                    let mut slot = inner.lock().await;
                    if let Some(worker) = slot.as_ref()
                        && worker.should_retire(&config, Instant::now())
                    {
                        tracing::info!("Retiring local node executor");
                        // In-flight invocations retain their own Arc until EOF/timeout.
                        // New requests lazily start a fresh process.
                        slot.take();
                    }
                }
            });
        }
        Ok(executor)
    }

    async fn retire_if_current(&self, worker: &Arc<InnerLocalNodeExecutor>) {
        let mut slot = self.inner.lock().await;
        if slot.as_ref().is_some_and(|current| Arc::ptr_eq(current, worker)) {
            slot.take();
        }
    }

    async fn invoke_owned(
        &self,
        request_json: JsonValue,
        log_line_sender: mpsc::UnboundedSender<LogLine>,
    ) -> anyhow::Result<InvokeResponse> {
        let worker = {
            let mut inner = self.inner.lock().await;
            if inner.is_none() {
                *inner = Some(
                    Arc::new(InnerLocalNodeExecutor::new(&self.config)
                        .await
                        .context("Failed to create inner local node executor")?),
                )
            }
            inner.as_ref().unwrap().clone()
        };
        let invocation = ActiveInvocation(worker);
        let worker = &invocation.0;

        let response_result = worker.client
            .post("http://localhost/invoke".to_string())
            .json(&request_json)
            .timeout(self.config.node_process_timeout)
            .send()
            .await;
        let response = match response_result {
            Ok(response) => response,
            Err(e) => {
                if e.is_timeout() {
                    return Ok(InvokeResponse {
                        response: EXECUTE_TIMEOUT_RESPONSE_JSON.clone(),
                        aws_request_id: None,
                    });
                } else if e.is_connect() {
                    // Connection error likely means the Node server crashed (e.g., OOM).
                    // Drop the dead server so it will be restarted on next invoke.
                    tracing::warn!("Node server connection failed, dropping server: {e}");
                    self.retire_if_current(worker).await;
                    return Err(anyhow::anyhow!(e).context("Node server request failed"));
                } else {
                    return Err(anyhow::anyhow!(e).context("Node server request failed"));
                }
            },
        };

        if let Err(e) = response.error_for_status_ref() {
            if e.status() == Some(reqwest::StatusCode::PAYLOAD_TOO_LARGE) {
                return Err(
                    anyhow::anyhow!(e.without_url()).context(ErrorMetadata::bad_request(
                        "ArgsTooLarge",
                        ARGS_TOO_LARGE_RESPONSE_MESSAGE,
                    )),
                );
            }
            let error = response.text().await?;
            anyhow::bail!("Node executor server returned error: {}", error);
        }
        let stream = Self::response_stream(&self.config, response);
        let stream = Box::pin(stream);
        let result = handle_node_executor_stream(log_line_sender, stream).await?;
        match result {
            Ok(payload) => {
                if payload
                    .get("exitingProcess")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    // Drop the server if it claims to be exiting.
                    self.retire_if_current(worker).await;
                }
                Ok(InvokeResponse {
                    response: payload,
                    aws_request_id: None,
                })
            },
            Err(e) => Ok(e),
        }
    }

    #[try_stream(ok = NodeExecutorStreamPart, error = anyhow::Error)]
    async fn response_stream(config: &LocalNodeExecutorConfig, mut response: reqwest::Response) {
        let mut timeout_future = Box::pin(tokio::time::sleep(config.node_process_timeout));
        let timeout_future = &mut timeout_future;
        loop {
            let process_chunk = async {
                select_biased! {
                    chunk = response.chunk().fuse() => {
                        let chunk = chunk?;
                        match chunk {
                            Some(chunk) => {
                                anyhow::Ok(NodeExecutorStreamPart::Chunk(chunk))
                            }
                            None => {
                                anyhow::Ok(NodeExecutorStreamPart::InvokeComplete(
                                    InvokeCompletion::Success,
                                ))
                            }
                        }
                    },
                    _ = timeout_future.fuse() => {
                        anyhow::Ok(NodeExecutorStreamPart::InvokeComplete(
                            InvokeCompletion::ExplicitError(InvokeResponse {
                                response: EXECUTE_TIMEOUT_RESPONSE_JSON.clone(),
                                aws_request_id: None,
                            }),
                        ))
                    },
                }
            };
            let part = process_chunk.await?;
            if let NodeExecutorStreamPart::InvokeComplete(_) = part {
                yield part;
                break;
            } else {
                yield part;
            }
        }
    }
}

#[async_trait]
impl NodeExecutor for LocalNodeExecutor {
    fn enable(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn invoke(
        &self,
        request: ExecutorRequest,
        log_line_sender: mpsc::UnboundedSender<LogLine>,
    ) -> anyhow::Result<InvokeResponse> {
        let request_json = JsonValue::try_from(request)?;
        let executor = Self {
            inner: self.inner.clone(),
            config: self.config.clone(),
        };
        tokio::spawn(async move {
            let (sender, mut receiver) = mpsc::unbounded_channel();
            let forward_logs = async move {
                while let Some(line) = receiver.recv().await {
                    // A disconnected caller must not abort the worker response drain.
                    let _ = log_line_sender.send(line);
                }
            };
            let (result, ()) = tokio::join!(
                executor.invoke_owned(request_json, sender),
                forward_logs,
            );
            result
        })
        .await
        .context("Local node executor invocation task failed")?
    }

    fn shutdown(&self) {}
}


#[cfg(test)]
mod tests {
    use std::process::Stdio;

    use tokio::io::{AsyncBufReadExt, BufReader};

    use super::*;

    fn config(idle: u64, lifetime: u64) -> Arc<LocalNodeExecutorConfig> {
        Arc::new(LocalNodeExecutorConfig {
            node_process_timeout: Duration::from_secs(10),
            callback_initial_backoff: None,
            idle_timeout: Duration::from_secs(idle),
            max_lifetime: Duration::from_secs(lifetime),
        })
    }

    // Real HTTP response bodies and a real child process, without requiring a
    // source package or any backend credentials. The fixture accepts any JSON.
    async fn worker() -> anyhow::Result<Arc<InnerLocalNodeExecutor>> {
        worker_with_marker(None).await
    }

    async fn worker_with_marker(marker: Option<&std::path::Path>) -> anyhow::Result<Arc<InnerLocalNodeExecutor>> {
        let source_dir = TempDir::new()?;
        let script = r#"
            const http = require('http');
            http.createServer((req, res) => {
                req.resume();
                req.on('end', () => {
                    res.writeHead(200);
                    res.write('{"ok":');
                    setTimeout(() => {
                        res.end('true}\n');
                        if (process.env.CROWWD_TEST_COMPLETED_FILE) {
                            require('fs').writeFileSync(process.env.CROWWD_TEST_COMPLETED_FILE, 'complete');
                        }
                    }, 150);
                });
            }).listen(0, '127.0.0.1', function () {
                console.log(this.address().port);
            });
        "#;
        let mut command = TokioCommand::new("node");
        command.arg("-e").arg(script).stdout(Stdio::piped()).kill_on_drop(true);
        if let Some(marker) = marker {
            command.env("CROWWD_TEST_COMPLETED_FILE", marker);
        }
        let mut child = command.spawn()?;
        let mut port = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut port)
            .await?;
        let port = port.trim().parse::<u16>()?;
        // Proxy to the fixture without binding localhost:80.
        let client = Client::builder()
            .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))?)
            .build()?;
        Ok(Arc::new(InnerLocalNodeExecutor {
            _source_dir: source_dir,
            client,
            _server_handle: child,
            started_at: Instant::now(),
            last_finished: StdMutex::new(Instant::now()),
        }))
    }

    #[test]
    fn disabled_and_reaper_deadlines() {
        assert_eq!(config(0, 0).reaper_period(), None);
        assert_eq!(config(1, 0).reaper_period(), Some(Duration::from_secs(1)));
        assert_eq!(config(300, 0).reaper_period(), Some(Duration::from_secs(30)));
        assert_eq!(config(300, 40).reaper_period(), Some(Duration::from_secs(10)));
    }

    #[tokio::test]
    async fn idle_uses_completion_and_lifetime_allows_active_drain() -> anyhow::Result<()> {
        let worker = worker().await?;
        let start = worker.started_at;
        assert!(!worker.should_retire(&config(0, 0), start + Duration::from_secs(100)));
        assert!(worker.should_retire(&config(5, 0), start + Duration::from_secs(6)));
        let active = ActiveInvocation(worker.clone());
        assert!(!worker.should_retire(&config(5, 0), start + Duration::from_secs(6)));
        assert!(worker.should_retire(&config(0, 5), start + Duration::from_secs(6)));
        drop(active);
        assert!(!worker.should_retire(&config(5, 0), Instant::now()));
        Ok(())
    }

    #[tokio::test]
    async fn old_failure_cannot_retire_replacement() -> anyhow::Result<()> {
        let old = worker().await?;
        let new = worker().await?;
        let executor = LocalNodeExecutor {
            inner: Arc::new(Mutex::new(Some(new.clone()))),
            config: config(0, 0),
        };
        executor.retire_if_current(&old).await;
        assert!(Arc::ptr_eq(executor.inner.lock().await.as_ref().unwrap(), &new));
        executor.retire_if_current(&new).await;
        assert!(executor.inner.lock().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn cancellation_and_streaming_drain_keep_retired_worker_alive() -> anyhow::Result<()> {
        let markers = TempDir::new()?;
        let marker = markers.path().join("completed");
        let worker = worker_with_marker(Some(&marker)).await?;
        let weak_worker = Arc::downgrade(&worker);
        let executor = Arc::new(LocalNodeExecutor {
            inner: Arc::new(Mutex::new(Some(worker))),
            config: config(0, 0),
        });
        let (logs, log_receiver) = mpsc::unbounded_channel();
        drop(log_receiver);
        let caller = {
            let executor = executor.clone();
            tokio::spawn(async move {
                executor.invoke(ExecutorRequest::BuildDeps(crate::BuildDepsRequest {
                    deps: vec![],
                    upload_url: String::new(),
                }), logs).await
            })
        };
        // Wait for the owned invocation to acquire its worker.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if weak_worker.strong_count() >= 2 { break; }
                tokio::task::yield_now().await;
            }
        }).await?;
        caller.abort();
        executor.inner.lock().await.take();
        assert!(weak_worker.upgrade().is_some());
        tokio::time::timeout(Duration::from_secs(5), async {
            while weak_worker.upgrade().is_some() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await?;
        assert_eq!(fs::read_to_string(marker)?, "complete");
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_owned_calls_complete_on_retired_worker() -> anyhow::Result<()> {
        let worker = worker().await?;
        let executor = Arc::new(LocalNodeExecutor {
            inner: Arc::new(Mutex::new(Some(worker))),
            config: config(0, 0),
        });
        let mut calls = vec![];
        for _ in 0..8 {
            let executor = executor.clone();
            calls.push(tokio::spawn(async move {
                let (logs, mut receiver) = mpsc::unbounded_channel();
                let (result, ()) = tokio::join!(
                    executor.invoke_owned(serde_json::json!({}), logs),
                    async { while receiver.recv().await.is_some() {} },
                );
                result
            }));
        }
        let retired = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let mut slot = executor.inner.lock().await;
                if Arc::strong_count(slot.as_ref().unwrap()) == 9 {
                    break slot.take().unwrap();
                }
                drop(slot);
                tokio::task::yield_now().await;
            }
        }).await?;
        for call in calls {
            assert_eq!(call.await??.response, serde_json::json!({"ok": true}));
        }
        assert_eq!(Arc::strong_count(&retired), 1);
        Ok(())
    }
}
