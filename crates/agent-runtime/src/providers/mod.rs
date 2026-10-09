//! Parallel implementations of the same provider contract.
pub(crate) mod claude;
pub(crate) mod codex;
pub(crate) mod crabot;
pub(crate) mod executable;
mod home;
mod launch_command;
pub(crate) mod mock;
pub(crate) mod opencode;
mod process;

mod provider;
pub(crate) use provider::Provider;
