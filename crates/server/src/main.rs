//! The `binge-server` binary.

mod api;
mod app;
mod built;
mod config;
mod health_check;
mod host;
mod ui;

use std::time::Duration;

use anyhow::{Context, Result};
use binge_core::{Accounts, Backups, Catalog, Database};
use clap::Parser;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use crate::app::AppState;
use crate::config::{Cli, Command, ServeArgs};
use crate::host::AllowedHosts;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Openapi) => print_openapi(),
        Some(Command::Health) => say_health(&cli.serve),
        None => serve(cli.serve).await,
    }
}

fn print_openapi() -> Result<()> {
    let json = api::description()
        .to_pretty_json()
        .context("could not render the API description")?;
    println!("{json}");
    Ok(())
}

/// Says whether the server that runs with these options is well, and fails
/// when it is not, which is what a health check goes by.
fn say_health(args: &ServeArgs) -> Result<()> {
    let version = health_check::ask(health_check::reached_at(args.address()))?;
    println!("Binge Server {version} is up");
    Ok(())
}

async fn serve(args: ServeArgs) -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let data_dir = args.data_dir()?;
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("could not create {}", data_dir.display()))?;
    let db = Database::open(&data_dir.join("binge.db")).await?;

    let address = args.address();
    let listener = TcpListener::bind(address)
        .await
        .with_context(|| format!("could not listen on {address}"))?;

    tracing::info!(
        version = binge_core::VERSION,
        commit = built::commit(),
        data_dir = %data_dir.display(),
        "Binge Server listening on http://{address}"
    );
    let accounts = Accounts::new(db.clone());
    if accounts.needs_setup().await? {
        tracing::info!("no owner yet: open the address above in a browser to create the account");
        if !args.bind.is_loopback() {
            tracing::warn!(
                "listening beyond this machine before the owner exists: \
                 whoever opens {address} first becomes the owner"
            );
        }
    }

    // Libraries may have changed while the server was not running.
    let catalog = Catalog::new(db.clone(), &data_dir);
    catalog.catch_up_credits().await?;
    catalog.start_scanning_all().await?;

    let backups = Backups::new(db.clone(), &data_dir);
    tokio::spawn(copy_on_schedule(backups.clone()));

    let state = AppState {
        db,
        catalog,
        accounts,
        backups,
        allowed_hosts: AllowedHosts::new(args.allowed_hosts),
    };
    axum::serve(listener, app::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server stopped unexpectedly")
}

/// How often the server looks whether a copy of the database is due.
const BETWEEN_LOOKS: Duration = Duration::from_secs(60 * 60);

/// How long after it starts the server first looks: once the scans that
/// start with it have had the database to themselves for a while.
const BEFORE_FIRST_LOOK: Duration = Duration::from_secs(5 * 60);

/// Makes a copy of the database when one is due, for as long as the server
/// runs. A copy that cannot be made is reported and tried again later.
async fn copy_on_schedule(backups: Backups) {
    tokio::time::sleep(BEFORE_FIRST_LOOK).await;
    loop {
        match backups.make_when_due().await {
            Ok(Some(copy)) => tracing::info!(copy = %copy.name, "copied the database"),
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "could not copy the database"),
        }
        tokio::time::sleep(BETWEEN_LOOKS).await;
    }
}

/// Resolves when the process is asked to stop. A signal that cannot be
/// listened for is reported and then never fires; it must not stop the server.
async fn shutdown_signal() {
    let interrupt = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(%error, "cannot listen for the interrupt signal");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(error) => {
                tracing::warn!(%error, "cannot listen for the terminate signal");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => {},
        () = terminate => {},
    }
    tracing::info!("shutting down");
}
