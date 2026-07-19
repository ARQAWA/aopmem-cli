pub mod adapter;
pub mod artifacts;
pub mod audit;
pub mod audit_repair;
pub mod cli;
pub mod install;
pub mod mutation;
pub mod observability;
pub mod output;
mod platform_check;
mod platform_publish;
pub mod recall;
pub mod redaction;
pub mod reflection;
pub mod schema;
pub mod storage;
pub mod task;
pub mod tools;
pub mod ui;
pub mod verify;
mod windows_path;

fn main() -> std::process::ExitCode {
    crate::cli::run()
}
