pub mod adapter;
pub mod artifacts;
pub mod audit;
pub mod cli;
pub mod install;
pub mod mutation;
pub mod observability;
pub mod output;
pub mod recall;
pub mod reflection;
pub mod schema;
pub mod storage;
pub mod tools;
pub mod ui;
pub mod upgrade;
pub mod verify;

fn main() -> std::process::ExitCode {
    crate::cli::run()
}
