use clap::Parser;
use kursal_core::apiserver::ApiDoc;
use std::path::PathBuf;
use utoipa::OpenApi;

#[derive(Parser, Debug)]
pub struct CLIArgs {
    /// Where to write the output file
    #[arg(short, long, default_value = "dist/openapi.json")]
    pub out: PathBuf,
}

pub fn main() {
    let args = CLIArgs::parse();
    let spec = ApiDoc::openapi();

    std::fs::write(args.out, spec.to_pretty_json().unwrap()).unwrap();
}
