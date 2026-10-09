use super::{controller::Controller, output};
use crate::management::Manager;
use std::{
    io::{IsTerminal, Write},
    sync::Arc,
};
use tokio::io::AsyncBufReadExt;

pub(crate) async fn run(manager: Arc<Manager>) -> Result<(), String> {
    if std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal()
        && std::env::var("TERM").ok().as_deref() != Some("dumb")
    {
        return super::tui::run(manager).await;
    }
    let mut controller = Controller::new(manager.clone()).await?;
    let (p, id, b) = controller.view();
    let mut watcher = output::watch(manager.clone(), p, id, b, false).await;
    let address = manager.web().address().await.map(|a| format!("http://{a}"));
    println!("{}", super::banner::text(address.as_deref(), 80));
    let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    loop {
        if controller.private_input() && std::io::stdin().is_terminal() {
            controller.cancel_process_input();
            eprintln!("此终端无法安全隐藏输入，请使用 Web 进程面板或正常交互终端。");
        }
        print!("{}> ", controller.label());
        std::io::stdout().flush().map_err(|e| e.to_string())?;
        let line = tokio::select! {
            line=lines.next_line()=>line.map_err(|e|e.to_string())?,
            _=tokio::signal::ctrl_c()=>{if let Err(e)=controller.interrupt().await {eprintln!("{e}");}continue;},
        };
        let Some(line) = line else {
            break;
        };
        if line.trim().is_empty() && !controller.choosing_chat() && !controller.private_input() {
            continue;
        }
        match controller.execute(&line).await {
            Ok(action) => {
                if action.exit {
                    break;
                }
                if action.configure {
                    watcher.abort();
                    let result = async {
                        let settings = crate::configuration::edit(
                            crate::configuration::Settings::load()?,
                            &mut lines,
                        )
                        .await?;
                        manager.reconfigure(settings).await
                    }
                    .await;
                    let (p, id, b) = controller.view();
                    watcher = output::watch(manager.clone(), p, id, b, false).await;
                    match result {
                        Ok(()) => println!("默认 Agent 配置已保存并生效；环境变量在重启时仍优先。"),
                        Err(e) => eprintln!("error: {e}"),
                    }
                } else {
                    if action.navigate {
                        watcher.abort();
                        let (p, id, b) = controller.view();
                        watcher = output::watch(manager.clone(), p, id, b, action.replay).await;
                    }
                    if !action.text.is_empty() {
                        println!("{}", action.text);
                    }
                }
            }
            Err(error) => eprintln!("error: {error}"),
        }
    }
    watcher.abort();
    Ok(())
}
