//! Plugin-wide state. Command handlers and IPC callbacks are separate entry
//! points that can't share `&mut self`, so the store lives in a global.
//!
//! RULE: never hold a guard across a call into the host (sending a message,
//! etc.). The host may re-enter this plugin synchronously and deadlock.
//! Copy what you need out of the guard, drop it, then call the host.

use crate::lang::Store;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub struct State {
    pub store: Store,
    /// The plugin's private data folder (as seen from inside the WASI sandbox).
    pub dir: PathBuf,
}

static SERVER: OnceLock<pumpkin_plugin_api::Server> = OnceLock::new();

/// Called once from `on_load`. Needed by the send/broadcast bridge.
pub fn set_server(server: pumpkin_plugin_api::Server) {
    let _ = SERVER.set(server);
}

pub fn server() -> Option<&'static pumpkin_plugin_api::Server> {
    SERVER.get()
}

static STATE: OnceLock<RwLock<State>> = OnceLock::new();

fn cell() -> &'static RwLock<State> {
    STATE.get_or_init(|| {
        RwLock::new(State {
            store: Store::new(),
            dir: PathBuf::new(),
        })
    })
}

pub fn read() -> RwLockReadGuard<'static, State> {
    cell().read().unwrap_or_else(|e| e.into_inner())
}

pub fn write() -> RwLockWriteGuard<'static, State> {
    cell().write().unwrap_or_else(|e| e.into_inner())
}
