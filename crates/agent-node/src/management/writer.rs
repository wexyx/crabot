use super::session;
use crate::core::Core;
use serde_json::Value;
use tokio::sync::mpsc;
use uuid::Uuid;

pub(super) enum Write {
    Event(Value),
    Barrier(tokio::sync::oneshot::Sender<Result<(), String>>),
    Finish { event: Value, status: &'static str },
}
pub(super) async fn run(
    core: Core,
    id: Uuid,
    journal: super::journal::Journal,
    mut rx: mpsc::UnboundedReceiver<Write>,
) -> Result<Value, String> {
    let mut batch = Vec::new();
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(200));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            message=rx.recv()=> match message {
                Some(Write::Barrier(reply))=>{
                    let result=if batch.is_empty(){Ok(())}else{session::append_batch(&core,id,std::mem::take(&mut batch),None).await.map(|_|())};
                    let _=reply.send(result.clone());result?;
                },
                Some(Write::Event(event))=>{
                    let boundary=event["type"]!="text_delta";
                    batch.push(event);
                    if boundary || batch.len()>=32 {let saved=session::append_batch(&core,id,std::mem::take(&mut batch),None).await?;if let Some(last)=saved.last(){journal.committed(id,last["seq"].as_u64().unwrap_or(0));}}
                },
                Some(Write::Finish{event,status})=>{
                    batch.push(event);
                    let events=session::append_batch(&core,id,batch,Some(status)).await?;
                    return events.last().cloned().ok_or("missing terminal event".into());
                },
                None=>return Err("management writer closed without final state".into()),
            },
            _=tick.tick(),if !batch.is_empty()=>{session::append_batch(&core,id,std::mem::take(&mut batch),None).await?;},
        }
    }
}
