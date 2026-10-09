use super::{application, startup};
pub fn run() {
    let interactive = match startup::configure() {
        Ok(None) => return,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
        Ok(Some(interactive)) => interactive,
    };
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("create runtime")
        .block_on(async {
            let dir = agent_runtime::paths::data_dir();
            let store = match crate::storage::open(&dir).await {
                Ok(store) => store,
                Err(error) => {
                    eprintln!("Cannot start Crabot instance: {error}");
                    std::process::exit(2);
                }
            };
            if let Err(error) = agent_runtime::prompts::PromptStore::new(&dir).validate_all() {
                eprintln!("Cannot load system prompts: {error}");
                std::process::exit(2);
            }
            match crate::configuration::startup().await {
                Ok(config) => application::serve(interactive, config, store).await,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(2);
                }
            }
        });
}
