use crate::{
    config::RelayConfig,
    swarm::{RelaySnapshot, spawn_relay_swarm},
};
use clap::Parser;
use kursal_core::logging::init_logging;
use kursal_core::stats::{StatsCollector, new_shared_registry};
use std::path::PathBuf;
use tokio::sync::watch;

pub mod config;
pub mod health;
pub mod identity;
pub mod swarm;
pub mod tui;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, author)]
pub struct CLIArgs {
    /// Config path for the relay (toml)
    #[arg(short, long, default_value = "relay.toml")]
    pub config: PathBuf,

    /// Returns success if the config file is valid
    #[arg(long)]
    pub validate: bool,

    /// Initiates a new toml config if none exists at the config path
    #[arg(long)]
    pub default_config: bool,

    /// Live terminal dashboard (connections, circuits, traffic, cpu)
    #[arg(long)]
    pub tui: bool,
}

const DEFAULT_RELAY_TOML: &str = include_str!("../relay.example.toml");

pub async fn run(config: PathBuf, validate: bool, default_config: bool, tui: bool) {
    if default_config
        && !std::fs::exists(&config)
            .expect("Cannot provide --default-config flag if config file already exists.")
    {
        std::fs::create_dir_all(config.parent().expect("Could not get the parent directory"))
            .expect("Could not create parent directory");
        std::fs::write(&config, DEFAULT_RELAY_TOML).expect("Could not write file");
    }

    let relay_config = RelayConfig::load(&config).expect("Could not parse relay config. Does the file exist? If not, provide the --default-config flag to create it.");

    if !tui || relay_config.log_file.is_some() {
        init_logging(&relay_config.log_level, relay_config.log_file.as_deref())
            .expect("Could not initiate logging");
    }

    if validate {
        println!("Valid config file!");
        return;
    }

    let keypair_path = config
        .parent()
        .expect("config has no parent dir")
        .join("relay_identity.key");

    let registry = new_shared_registry();

    if tui {
        let (snapshot_tx, snapshot_rx) = watch::channel(RelaySnapshot::default());
        let swarm_task = tokio::spawn(spawn_relay_swarm(
            relay_config,
            keypair_path,
            registry.clone(),
            Some(snapshot_tx),
        ));

        let collector = StatsCollector::new(registry);
        let ui = tokio::task::spawn_blocking(move || tui::run(snapshot_rx, collector));
        let _ = ui.await;
        swarm_task.abort();
    } else {
        spawn_relay_swarm(relay_config, keypair_path, registry, None)
            .await
            .expect("Could not spawn swarm");
    }
}
