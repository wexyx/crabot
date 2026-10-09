use super::{SessionScope, output::Output, pty::Pty};
use crate::{
    execution::native::{NativeCommand, ProcessGroup},
    skills::SkillBridge,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Mutex as AsyncMutex, watch},
};

enum Input {
    Pipe(tokio::process::ChildStdin),
    Terminal(Arc<Pty>),
}
pub(super) struct Process {
    id: String,
    owner: SessionScope,
    command: String,
    root: PathBuf,
    terminal: bool,
    input: AsyncMutex<Input>,
    output: Mutex<Output>,
    stop: watch::Sender<bool>,
    activity: Mutex<Instant>,
    finished: watch::Sender<bool>,
}
impl Process {
    pub(super) fn launch(
        owner: SessionScope,
        root: PathBuf,
        script: String,
        terminal: bool,
        env: BTreeMap<String, String>,
        bridge: Option<SkillBridge>,
    ) -> Result<Arc<Self>, String> {
        let resources = NativeCommand::new(&root)?;
        let mut cmd = resources.shell_command(&root, &owner.home(), terminal)?;
        cmd.arg("-c").arg(&script).envs(env);
        let pty = if terminal {
            Some(Arc::new(Pty::attach(&mut cmd).map_err(|e| e.to_string())?))
        } else {
            cmd.stdin(std::process::Stdio::piped());
            None
        };
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("command launch failed: {e}"))?;
        let group = ProcessGroup::new(child.id().ok_or("missing process ID")?);
        let input = if let Some(pty) = &pty {
            Input::Terminal(pty.clone())
        } else {
            Input::Pipe(child.stdin.take().ok_or("missing stdin")?)
        };
        let (stop, mut stopped) = watch::channel(false);
        let (finished, _) = watch::channel(false);
        let process = Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            owner,
            command: script,
            root,
            terminal,
            input: AsyncMutex::new(input),
            output: Mutex::new(Output::new()),
            stop,
            activity: Mutex::new(Instant::now()),
            finished,
        });
        let mut readers = tokio::task::JoinSet::new();
        let terminal_cleanup = pty.clone();
        if let Some(pty) = pty {
            let p = process.clone();
            readers.spawn(async move {
                let mut bytes = [0; 4096];
                loop {
                    match pty.read(&mut bytes).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => p.output.lock().unwrap().append(false, &bytes[..n]),
                    }
                }
            });
        } else {
            fn reader<T: tokio::io::AsyncRead + Unpin + Send + 'static>(
                mut io: T,
                p: Arc<Process>,
                err: bool,
            ) -> impl std::future::Future<Output = ()> + Send {
                async move {
                    let mut bytes = [0; 4096];
                    loop {
                        match io.read(&mut bytes).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => p.output.lock().unwrap().append(err, &bytes[..n]),
                        }
                    }
                }
            }
            readers.spawn(reader(
                child.stdout.take().ok_or("missing stdout")?,
                process.clone(),
                false,
            ));
            readers.spawn(reader(
                child.stderr.take().ok_or("missing stderr")?,
                process.clone(),
                true,
            ));
        }
        let p = process.clone();
        tokio::spawn(async move {
            let _resources = resources;
            let _bridge = bridge;
            let mut tick = tokio::time::interval(Duration::from_secs(30));
            let (status, code) = loop {
                tokio::select! {
                    result=child.wait()=>break match result{Ok(s)=>("completed",s.code()),Err(_)=>("failed",None)},
                    _=stopped.changed()=>{let _=child.start_kill();break ("cancelled",None)},
                    _=tick.tick()=>if p.activity.lock().unwrap().elapsed()>Duration::from_secs(1800){let _=child.start_kill();break ("expired",None)},
                }
            };
            if let Some(pty) = terminal_cleanup {
                pty.stop_foreground();
            }
            drop(group); // Also close inherited pipes held by descendants.
            let _ = child.wait().await;
            let _ = tokio::time::timeout(Duration::from_millis(500), async {
                while readers.join_next().await.is_some() {}
            })
            .await;
            readers.abort_all();
            p.output.lock().unwrap().finish(status, code);
            p.finished.send_replace(true);
        });
        Ok(process)
    }
    pub(super) fn id(&self) -> &str {
        &self.id
    }
    pub(super) fn owner(&self) -> &SessionScope {
        &self.owner
    }
    pub(super) fn root(&self) -> &std::path::Path {
        &self.root
    }
    pub(super) fn command(&self) -> &str {
        &self.command
    }
    pub(super) fn running(&self) -> bool {
        self.output.lock().unwrap().running()
    }
    pub(super) fn stop(&self) {
        self.stop.send_replace(true);
    }
    pub(super) async fn wait(&self, ms: u64) {
        let mut done = self.finished.subscribe();
        if !*done.borrow_and_update() {
            let _ =
                tokio::time::timeout(Duration::from_millis(ms.min(10_000)), done.changed()).await;
        }
    }
    pub(super) async fn write(&self, data: &str, private: bool) -> Result<(), String> {
        if !self.running() {
            return Err("process already exited".into());
        }
        if data.len() > 8192 {
            return Err("input maximum 8192 bytes".into());
        }
        if private {
            self.output.lock().unwrap().private();
        }
        *self.activity.lock().unwrap() = Instant::now();
        tokio::time::timeout(Duration::from_secs(5), async {
            match &mut *self.input.lock().await {
                Input::Pipe(io) => io.write_all(data.as_bytes()).await,
                Input::Terminal(pty) => pty.write(data.as_bytes()).await,
            }
        })
        .await
        .map_err(|_| "process input timed out")?
        .map_err(|e| e.to_string())
    }
    pub(super) fn view(&self, after: u64, human: bool) -> Value {
        let mut v = self.output.lock().unwrap().view(after, human);
        v["session_id"] = json!(self.id);
        v["agent"] = json!(self.owner.agent());
        v["command"] = json!(self.command);
        v["workdir"] = json!(self.root);
        v["interactive"] = json!(self.terminal);
        v["execution"] = json!("host");
        v["network"] = json!("host");
        v
    }
}
