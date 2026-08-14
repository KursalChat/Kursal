use crate::{config::RelayConfig, swarm::spawn_relay_swarm};
use clap::Parser;
use kursal_core::logging::init_logging;
use kursal_core::stats::new_shared_registry;
use std::path::PathBuf;

#[cfg(feature = "tui")]
use crate::swarm::RelaySnapshot;
#[cfg(feature = "tui")]
use kursal_core::stats::StatsCollector;
#[cfg(feature = "tui")]
use tokio::sync::watch;

pub mod config;
pub mod health;
pub mod identity;
pub mod swarm;
#[cfg(feature = "tui")]
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

pub async fn run(
    config: PathBuf,
    validate: bool,
    default_config: bool,
    tui: bool,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let parent = config
        .parent()
        .ok_or_else(|| format!("config path `{}` has no parent directory", config.display()))?;

    if default_config && !std::fs::exists(&config)? {
        std::fs::create_dir_all(parent)?;
        std::fs::write(&config, DEFAULT_RELAY_TOML)?;
    }

    let relay_config = RelayConfig::load(&config).map_err(|err| {
        format!(
            "Could not parse relay config at `{}`: {err}. If the file does not exist, provide the --default-config flag to create it.",
            config.display()
        )
    })?;

    if !tui || relay_config.log_file.is_some() {
        init_logging(&relay_config.log_level, relay_config.log_file.as_deref())?;
    }

    if validate {
        println!("Valid config file!");
        return Ok(());
    }

    let keypair_path = parent.join("relay_identity.key");

    let registry = new_shared_registry();

    #[cfg(feature = "tui")]
    {
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
            spawn_relay_swarm(relay_config, keypair_path, registry, None).await?;
        }
    }

    #[cfg(not(feature = "tui"))]
    {
        if tui {
            return Err("this build has no terminal dashboard (`tui` feature disabled)".into());
        }
        spawn_relay_swarm(relay_config, keypair_path, registry, None).await?;
    }

    Ok(())
}
