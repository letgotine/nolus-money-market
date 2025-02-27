#![cfg(all(test, not(target_arch = "wasm32")))]

mod common;
mod dispatcher_tests;
mod lease;
mod leaser_tests;
mod lpp_tests;
mod oracle_tests;
mod profit_tests;
mod rust_runtime_tests;
mod timealarms_tests;
