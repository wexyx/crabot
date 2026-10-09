use super::{
    controller::Controller,
    editor::Editor,
    output::{self, Update},
    presentation::Presentation,
    screen::Screen,
};
use crate::{
    configuration::{Settings, Wizard},
    management::Manager,
};
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) async fn run(manager: Arc<Manager>) -> Result<(), String> {
    let mut controller = Controller::new(manager.clone()).await?;
    let mut session_info = controller.session_info().await;
    let (p, id, b) = controller.view();
    let (mut updates, mut watcher) = output::subscribe(manager.clone(), p, id, b, false).await;
    let _screen = Screen::enter()?;
    let mut keyboard = EventStream::new();
    let mut editor = Editor::default();
    let address = manager.web().address().await.map(|a| format!("http://{a}"));
    _screen.banner(address.as_deref())?;
    let mut transcript = String::new();
    let mut tools = super::tool_timeline::ToolTimeline::default();
    let mut presentation = Presentation::default();
    let mut dialog = super::permission_dialog::PermissionDialog::default();
    let (actions_tx, mut actions_rx) = tokio::sync::mpsc::unbounded_channel();
    let (decisions_tx, mut decisions_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut submitting = false;
    let mut pending_echo: Option<String> = None;
    let mut submission: Option<tokio::task::JoinHandle<()>> = None;
    let mut decision_job: Option<tokio::task::JoinHandle<()>> = None;
    let mut wizard: Option<Wizard> = None;
    let mut busy = false;
    let mut started = Instant::now();
    let mut dirty = true;
    let mut exit_armed: Option<Instant> = None;
    let mut scroll = 0usize;
    let mut status = String::new();
    // Refreshed on navigation so `@` completes against the group's current roster.
    let mut mentions: Vec<String> = Vec::new();
    let mut mention_group = String::new();
    let mut update_status = super::updater::subscribe();
    let update_check = tokio::spawn(super::updater::check());
    let mut tick = tokio::time::interval(Duration::from_millis(40));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let shutdown = crate::app::shutdown_signal();
    tokio::pin!(shutdown);
    let result:Result<(),String>=async {
        loop {
            tokio::select! {
                _=&mut shutdown=>break,
                result=update_status.changed()=>{if result.is_ok(){dirty=true;}},
                item=keyboard.next()=>{
                    let Some(event)=item else {break;};
                    if mention_group!=controller.group().unwrap_or_default(){mention_group=controller.group().unwrap_or_default();mentions=controller.mentionable().await;}
                    match event.map_err(|e|e.to_string())? {
                        Event::Resize(_,_)=>dirty=true,

                        Event::Paste(text)=>{if dialog.visible()&&wizard.is_none(){dialog.paste(&text);}else{editor.insert(&text);}dirty=true;},
                        Event::Key(key) if key.kind!=KeyEventKind::Release=>{
                            dirty=true;
                            let ctrl=key.modifiers.contains(KeyModifiers::CONTROL);
                            if ctrl&&key.code==KeyCode::Char('p'){dialog.show();continue;}
                            // Tab completes an `@name` this group can be addressed by,
                            // before it falls through to slash-command completion.
                            if key.code==KeyCode::Tab&&wizard.is_none()&&!dialog.visible() {
                                if let Some(fragment)=editor.mention_prefix()
                                    && let Some(name)=mentions.iter().find(|m|m.starts_with(fragment.as_str())).cloned()
                                    {editor.complete_mention(&name);continue;}
                            }
                            if dialog.visible()&&wizard.is_none() {
                                if decision_job.is_some(){status="权限请求正在处理，请稍候。".into();continue;}
                                if let Some((permission,allow,conversation))=dialog.key(key){
                                    let manager=manager.clone();let tx=decisions_tx.clone();
                                    status="正在处理权限确认…".into();
                                    decision_job=Some(tokio::spawn(async move{
                                        let result=if conversation{agent_runtime::workspace::allow_conversation(permission.id)}else if permission.workspace{agent_runtime::workspace::decide(permission.id,allow)}else{manager.core().decide(permission.project,permission.id,allow).await.map(|_|())};
                                        let _=tx.send(result.map(|_|if conversation{"本对话后续命令已允许；命令具有当前系统用户权限，重置上下文或重启后失效。".to_string()}else if allow{"已允许一次，操作继续执行。".to_string()}else{"已拒绝此操作。".to_string()}));
                                    }));
                                }
                                continue;
                            }
                            if !(ctrl&&key.code==KeyCode::Char('c')) {exit_armed=None;}
                            if let Some(config)=wizard.as_mut() {
                                if editor.text().is_empty() {
                                    let direction=match key.code {KeyCode::Up|KeyCode::PageUp=>Some(false),KeyCode::Down|KeyCode::PageDown|KeyCode::Tab=>Some(true),_=>None};
                                    if direction.is_some_and(|forward|config.move_choice(forward)){continue;}
                                }
                            }
                            if wizard.is_none() && editor.text().is_empty() {
                                let direction=match key.code {KeyCode::Up|KeyCode::PageUp=>Some(false),KeyCode::Down|KeyCode::PageDown=>Some(true),_=>None};
                                if direction.is_some_and(|forward|controller.move_chat_choice(forward)){continue;}
                            }
                            match key.code {

                                KeyCode::Char('l') if ctrl=>{transcript.clear();tools=Default::default();scroll=0;continue;},
                                KeyCode::Esc|KeyCode::Char('c') if key.code==KeyCode::Esc||ctrl=>{
                                    if wizard.take().is_some(){editor.clear();status="配置已取消，未保存。".into();}
                                    else if controller.cancel_chat_choice(){editor.clear();status="已取消选择聊天。".into();}
                                    else if busy {controller.interrupt().await?;status="正在打断…".into();}
                                    else if !editor.text().is_empty(){editor.clear();exit_armed=Some(Instant::now());status="输入已清空；再按 Ctrl+C 退出。".into();}
                                    else if ctrl && !submitting && controller.label()=="admin" && controller.can_back() {
                                        let mut next=controller.clone();let tx=actions_tx.clone();submitting=true;exit_armed=None;
                                        submission=Some(tokio::spawn(async move{let result=next.execute("/back").await;let _=tx.send((next,result));}));
                                    }
                                    else if ctrl {if exit_armed.is_some_and(|at|at.elapsed()<Duration::from_secs(2)){break;}exit_armed=Some(Instant::now());status="再按一次 Ctrl+C 退出，或继续输入。".into();}
                                    continue;
                                },
                                KeyCode::Char('d') if ctrl&&editor.text().is_empty()=>break,
                                KeyCode::Enter if !key.modifiers.intersects(KeyModifiers::ALT|KeyModifiers::SHIFT)=>{
                                    if wizard.is_none() && editor.complete_partial() { continue; }
                                    let line=editor.submit(wizard.is_none());
                                    if let Some(config)=wizard.as_mut() {
                                        if line.trim()=="/cancel" {wizard=None;status="配置已取消。".into();continue;}
                                        match config.submit(line).await {
                                            Ok(Some(settings))=>match manager.reconfigure(settings).await {
                                                Ok(())=>{wizard=None;session_info=controller.session_info().await;status="默认 Agent 配置已保存并生效。".into();},
                                                Err(e)=>status=e,
                                            },
                                            // A numbered model list belongs in the scrollback,
                                            // which this inline REPL keeps, not in the status line.
                                            Ok(None)=>{let listing=config.catalog_text();if !listing.is_empty(){transcript.push_str(&format!("\n{listing}"));}status=config.prompt()},
                                            Err(e)=>{transcript.push_str(&format!("\n配置无效：{e}\n"));status=e;},
                                        }
                                        continue;
                                    }
                                    let choosing_chat=controller.choosing_chat();
                                    if line.trim().is_empty()&&!choosing_chat{continue;}
                                    if let Ok(super::commands::Command::Tools(index))=super::commands::parse(&line){transcript.push_str(&format!("\n{}\n",tools.details(index)));continue;}
                                    if submitting {editor.insert(&line);status="上一条请求正在提交，请稍候。".into();continue;}
                                    if busy&&!choosing_chat&&!line.trim_start().starts_with('/') {editor.insert(&line);status="当前任务仍在运行，Esc 打断后可修改要求。".into();continue;}
                                    transcript.push_str(&format!("\n你：{line}\n"));scroll=0;
                                    if !choosing_chat&&!line.trim_start().starts_with('/'){pending_echo=Some(line.clone());busy=true;started=Instant::now();}
                                    status="正在提交…".into();submitting=true;
                                    _screen.draw(&controller.heading(),&transcript,&status,&session_info,&update_status.borrow().clone(),&editor,false,scroll,None,&tools)?;
                                    let mut next=controller.clone();let tx=actions_tx.clone();
                                    submission=Some(tokio::spawn(async move{let result=next.execute(&line).await;let _=tx.send((next,result));}));

                                },
                                _=>editor.key(key,wizard.is_some()),
                            }
                        },
                        _=>{},
                    }
                },
                Some((next,result))=actions_rx.recv()=>{
                    submitting=false;dirty=true;
                    match result{
                        Ok(action)=>{
                            controller=next;
                            session_info=controller.session_info().await;
                            if action.exit{break;}
                            if action.configure{match Settings::load(){Ok(settings)=>{let mut config=Wizard::new(settings);config.refresh_catalog().await;status=config.prompt();wizard=Some(config);},Err(e)=>status=e};continue;}
                            if action.navigate{watcher.abort();let(p,id,b)=controller.view();(updates,watcher)=output::subscribe(manager.clone(),p,id,b,action.replay).await;presentation=Presentation::default();tools=Default::default();transcript.clear();busy=false;pending_echo=None;}
                            if action.sent {if busy{status="等待模型响应…".into();}}
                            else if action.navigate {status=action.text;}
                            else {status="就绪".into();if !action.text.is_empty()&&!controller.choosing_chat(){transcript.push_str(&format!("{}\n",action.text));}}
                        },
                        Err(e)=>{if pending_echo.take().is_some(){busy=false;}status="操作失败，可修改后重试。".into();transcript.push_str(&format!("\n错误：{e}\n"));},
                    }
                },
                Some(result)=decisions_rx.recv()=>{
                    dirty=true;decision_job=None;
                    status=match result{Ok(text)=>text,Err(error)=>{dialog.show();format!("权限处理失败：{error}")}};
                },
                Some(update)=updates.recv()=>{
                    dirty=true;
                    match update {
                        Update::Permissions(items)=>dialog.update(items),
                        Update::Notice(text)=>transcript.push_str(&format!("\n{text}\n")),
                        Update::Event(row)=>{
                            let event=row.get("payload").unwrap_or(&row);
                            match event["type"].as_str().unwrap_or_default() {
                                "user"|"message.created"=>{tools.begin_turn();busy=true;started=Instant::now();let text=event["content"].as_str().unwrap_or_default();if pending_echo.as_deref()==Some(text){pending_echo=None;}else{transcript.push_str(&format!("\n你：{text}\n"));}},
                                "context_checkpoint"|"agent.context"=>{if event["content"].as_str().unwrap_or_default().starts_with("Context compacted:"){status="上下文已自动压缩 · 原始日志保留".into();}},
                                "agent.progress"=>status=format!("执行中 · {}",event["agent"].as_str().unwrap_or("Agent")),
                                "tool_started"|"agent.tool.started"=>status=format!("调用工具：{}",event["name"].as_str().unwrap_or("tool")),
                                "text_delta"|"agent.delta"=>{busy=true;status="正在回答…".into();},
                                "completed"|"agent.done"=>{busy=false;status=format!("完成 · {:.1}s",started.elapsed().as_secs_f64());},
                                "failed"|"agent.error"|"task.interrupted"=>{busy=false;status="任务已结束，可继续输入。".into();},
                                _=>{},
                            }
                            let rendered=presentation.render(&row);
                            transcript.push_str(&tools.record(&row).unwrap_or(rendered));
                        },
                    }
                    if transcript.len()>200_000 {let mut at=transcript.len()-150_000;while !transcript.is_char_boundary(at){at+=1;}transcript.drain(..at);}
                },
                _=tick.tick()=>{
                    if dirty||busy {
                        let secret=wizard.as_ref().is_some_and(|w|w.field().secret);
                        let heading=wizard.as_ref().map(|w|w.prompt()).unwrap_or_else(||controller.heading());
                        let shown=if let Some(w)=wizard.as_ref(){w.prompt()}else if busy {let tool=tools.status((started.elapsed().as_secs()/2) as usize).map(|value|format!(" · {}",value.split_once(" · ").map(|(_,name)|name).unwrap_or(&value))).unwrap_or_default();format!("执行中 · {:>6.1}s · Esc 打断{}",started.elapsed().as_secs_f64(),tool)}else{status.clone()};
                        let update=update_status.borrow().clone();
                        let panel=if let Some(config)=wizard.as_ref(){let menu=config.choice_menu();(!menu.is_empty()).then_some(menu)}else{dialog.text().or_else(||if editor.text().starts_with('/'){None}else{controller.chat_menu()})};
                        _screen.draw(&heading,&transcript,&shown,&session_info,&update,&editor,secret,scroll,panel.as_deref(),&tools)?;dirty=false;
                    }
                }
            }
        }
        Ok(())
    }.await;
    watcher.abort();
    update_check.abort();
    if let Some(job) = submission {
        job.abort();
    }
    if let Some(job) = decision_job {
        job.abort();
    }
    result
}
