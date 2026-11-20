//! Command line and environment configuration.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use directories::ProjectDirs;

#[derive(Debug, Parser)]
#[command(name = "binge-server", version, about = "Binge comic server")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub serve: ServeArgs,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print the OpenAPI description of the HTTP API and exit.
    Openapi,
    /// Ask the server running on this machine whether it is well. Exits
    /// with 0 when it is and with 1 when it is not, or when none answers.
    /// It asks where `--bind` and `--port` say the server listens.
    Health,
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Folder for the database and everything else the server writes.
    /// Defaults to the platform's application data folder.
    #[arg(long, env = "BINGE_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// Address to listen on. The default accepts connections from this
    /// machine only.
    #[arg(long, env = "BINGE_BIND", global = true, default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST))]
    pub bind: IpAddr,

    /// Port to listen on.
    #[arg(long, env = "BINGE_PORT", global = true, default_value_t = 8686)]
    pub port: u16,

    /// Extra host name the server answers to, such as `nas.local`. Repeat the
    /// option or separate names with commas. IP addresses and `localhost`
    /// always work.
    #[arg(
        long = "allowed-host",
        env = "BINGE_ALLOWED_HOSTS",
        value_name = "NAME",
        value_delimiter = ','
    )]
    pub allowed_hosts: Vec<String>,
}

impl ServeArgs {
    pub fn data_dir(&self) -> Result<PathBuf> {
        if let Some(dir) = &self.data_dir {
            return Ok(dir.clone());
        }
        let dirs = ProjectDirs::from("", "", "binge-server")
            .context("could not find a home folder; pass --data-dir")?;
        Ok(dirs.data_dir().to_path_buf())
    }

    pub fn address(&self) -> SocketAddr {
        SocketAddr::new(self.bind, self.port)
    }
}
