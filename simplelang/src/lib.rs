mod bridge;
mod catalogue;
mod host;
mod ipc;
mod lang;
mod state;
mod storage;

use pumpkin_plugin_api::{Context, Plugin, PluginMetadata};
use std::path::PathBuf;
use tracing::{info, warn};

struct SimpleLang;

impl Plugin for SimpleLang {
    fn new() -> Self {
        SimpleLang
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            // This name is also the id other plugins send IPC messages to.
            name: "simplelang".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            authors: vec!["SimpleLang".into()],
            description: "Per-player language selection with English fallback".into(),
            dependencies: vec![],
            // Needed to read/write the plugin's private data folder.
            permissions: vec!["fs.read.data".into(), "fs.write.data".into()],
        }
    }

    fn on_load(&self, context: Context) -> pumpkin_plugin_api::Result<()> {
        let dir = PathBuf::from(context.get_data_folder());

        storage::seed_defaults(&dir);
        storage::seed_config(&dir);

        {
            let mut st = state::write();
            st.dir = dir.clone();

            // 1) bundled defaults: lowest priority, guarantees every built-in key exists
            for (code, json) in storage::BUNDLED {
                match storage::parse_entries(json) {
                    Ok(entries) => st.store.register_raw(code, entries),
                    Err(e) => warn!("bundled {code}.json is invalid: {e}"),
                }
            }
            // Minecraft's language list: makes every language selectable with /lang
            match catalogue::load() {
                Ok(entries) => st.store.set_catalogue(entries),
                Err(e) => warn!("bundled minecraft_languages.json is invalid: {e}"),
            }
            // 2) admin-editable files: these win
            st.store.set_files(storage::load_files(&dir));
            st.store.set_aliases(storage::load_aliases(&dir));
            // 3) saved player choices
            st.store.set_prefs(storage::load_prefs(&dir));
        }

        // 4) config.json: the server-wide default language. Applied after the languages
        // are loaded so names like "portuguese" can be resolved. Logged outside the lock.
        let problems = {
            let config = storage::load_config(&dir);
            let mut st = state::write();
            storage::apply_config(&mut st.store, &config)
        };
        for problem in &problems {
            warn!("config.json: {problem}");
        }
        let (current_default, restricted) = {
            let st = state::read();
            (st.store.server_default().map(str::to_string), st.store.is_restricted())
        };
        match current_default {
            Some(code) => info!("default language for players who haven't chosen: {code}"),
            None => info!("default language: follows each player's game language (\"auto\")"),
        }
        if restricted {
            info!("supported languages are limited by config.json (see /lang list)");
        }

        // [F] Server handle for the send/broadcast bridge.
        state::set_server(context.get_server());

        host::register(&context)?;

        info!(
            "SimpleLang ready: {} language(s) loaded",
            state::read().store.available().len()
        );
        Ok(())
    }

    fn handle_ipc_message(
        &self,
        sender: String,
        message: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        ipc::handle(&sender, &message)
    }
}

pumpkin_plugin_api::register_plugin!(SimpleLang);
