use clap::Parser;
use kursal_cli::CLIArgs;

#[tokio::main]
pub async fn main() -> std::process::ExitCode {
    let args = CLIArgs::parse();

    match kursal_cli::run(args.config, args.validate, args.default_config, args.tui).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            std::process::ExitCode::FAILURE
        }
    }
}
