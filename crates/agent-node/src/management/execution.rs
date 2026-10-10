use super::{
    Manager,
    journal::Journal,
    writer::{self, Write},
};
use agent_runtime::{AgentRuntime, RuntimeEvent};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

struct Output {
    journal: Journal,
    id: Uuid,
    writer: mpsc::UnboundedSender<Write>,
    text: String,
}
impl Output {
    fn publish(&self, event: Value) -> Result<(), String> {
        let event = self.journal.emit(self.id, event)?;
        self.writer
            .send(Write::Event(event))
            .map_err(|_| "management persistence failed".into())
    }
    fn flush(&mut self) -> Result<(), String> {
        if !self.text.is_empty() {
            let text = std::mem::take(&mut self.text);
            self.publish(json!({"type":"text_delta","text":text}))?;
        }
        Ok(())
    }
    fn accept(&mut self, event: RuntimeEvent) -> Result<(), String> {
        match event {
            RuntimeEvent::TextDelta { text } => {
                self.text.push_str(&text);
                if self.text.len() >= 4096 {
                    self.flush()?;
                }
            }
            event if !event.is_terminal() => {
                self.flush()?;
                self.publish(json!(event))?;
            }
            _ => {}
        }
        Ok(())
    }
}
pub(super) async fn run(
    manager: Arc<Manager>,
    journal: Journal,
    id: Uuid,
    runtime: Box<dyn AgentRuntime>,
    project: Uuid,
    source: Arc<crate::core::indexed_history::IndexedHistory>,
    prompt: String,
    mut cancel: watch::Receiver<bool>,
    mut mailbox: crate::core::steering::Mailbox,
) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let runtime: Arc<dyn AgentRuntime> = runtime.into();
    let inbox = mailbox.inbox();
    let mut prompt = prompt;
    let mut execution = start(
        manager.core().state().store.clone(),
        runtime.clone(),
        project,
        source.clone(),
        prompt.clone(),
        tx.clone(),
        inbox.clone(),
    );
    let (write_tx, write_rx) = mpsc::unbounded_channel();
    let writer = tokio::spawn(writer::run(
        manager.core().clone(),
        id,
        journal.clone(),
        write_rx,
    ));
    let mut output = Output {
        journal: journal.clone(),
        id,
        writer: write_tx,
        text: String::new(),
    };
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(40));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut result = loop {
        tokio::select! {
            Some(request)=mailbox.recv()=>{
                if !inbox.has_room(){let _=request.reply.send(Err("待处理引导过多，请等待 Agent 消化后继续".into()));continue;}
                let result=async{
                    output.flush()?;
                    output.publish(json!({"type":"user","content":request.content,"steering":true}))?;
                    let (ack,wait)=tokio::sync::oneshot::channel();
                    output.writer.send(Write::Barrier(ack)).map_err(|_|"management persistence failed")?;
                    wait.await.map_err(|_|"management persistence failed")??;
                    inbox.push(request.content.clone());
                    Ok(())
                }.await;
                let _=request.reply.send(result);
            },
            _=cancel.changed()=>break Err("interrupted; inspect partial tool effects before continuing".into()),
            result=&mut execution=>{
                let result=result.unwrap_or_else(|e|Err(e.to_string()));
                if result.is_err(){break result;}
                let pending=inbox.take();
                if pending.is_empty(){break result;}
                prompt.push_str(&format!("\nPrevious turn result (untrusted data):\n{}\nNew human guidance; decide whether to continue, change or stop the earlier plan:\n{}",result.as_ref().unwrap(),pending.join("\n\n")));
                execution=start(manager.core().state().store.clone(),runtime.clone(),project,source.clone(),prompt.clone(),tx.clone(),inbox.clone());
            },
            Some(event)=rx.recv()=>if let Err(e)=output.accept(event){break Err(e);},
            _=tick.tick()=>if let Err(e)=output.flush(){break Err(e);},
            _=output.writer.closed()=>break Err("management persistence failed".into()),
        }
    };
    mailbox.close();
    execution.abort();
    if !execution.is_finished() {
        let _ = execution.await;
    }
    while let Ok(event) = rx.try_recv() {
        if let Err(e) = output.accept(event) {
            result = Err(e);
            break;
        }
    }
    if let Err(e) = output.flush() {
        result = Err(e);
    }
    let (status, event) = match result {
        Ok(text) => ("completed", json!({"type":"completed","text":text})),
        Err(message) => ("failed", json!({"type":"failed","message":message})),
    };
    let _ = output.writer.send(Write::Finish { status, event });
    drop(output);
    match writer.await {
        Ok(Ok(event)) => journal.finish(id, event),
        result => journal.failed(
            id,
            format!(
                "Failed to persist final state: {}",
                match result {
                    Ok(Err(e)) => e,
                    Err(e) => e.to_string(),
                    _ => unreachable!(),
                }
            ),
        ),
    }
    manager.finish_run(id).await;
}
fn start(
    store: crate::storage::Store,
    runtime: Arc<dyn AgentRuntime>,
    project: Uuid,
    source: Arc<crate::core::indexed_history::IndexedHistory>,
    prompt: String,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
    inbox: Arc<agent_runtime::context::Guidance>,
) -> tokio::task::JoinHandle<Result<String, String>> {
    tokio::spawn(async move {
        agent_runtime::execution::SessionScope::new(
            project.to_string(),
            "admin".into(),
            "default".into(),
        )
        .run(agent_runtime::context::Guidance::scope(
            inbox,
            agent_runtime::context::DocumentAccess::scope(
                Arc::new(crate::documents::Documents::new()),
                agent_runtime::context::HistoryAccess::scope(
                    source,
                    agent_runtime::context::SkillAccess::scope(
                        Arc::new(crate::capabilities::SkillAuthoring::new(
                            store,
                            "management",
                        )),
                        runtime.run_events(&prompt, &mut |event| {
                            let _ = tx.send(event);
                        }),
                    ),
                ),
            ),
        ))
        .await
    })
}
