#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod config;
mod format;
mod placeholders;
#[cfg(target_arch = "wasm32")]
mod runtime;
#[cfg(target_arch = "wasm32")]
mcsa_sdk::export_plugin!(runtime::CustomTabList);
