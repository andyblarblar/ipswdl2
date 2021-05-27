mod client;
mod apiJsonTypes;
mod downloader;

use structopt::*;
use std::path::{PathBuf, Path};
use tokio::macros::*;
use std::error::Error;
use crate::client::Client;
use log::{info, debug, error};
use crate::downloader::Downloader;

/// Downloads the newest .ipsw for Apple devices
#[derive(StructOpt)]
pub struct CliOpts {
    /// Directory to download .ipsw files to.
    #[structopt(short="p", long, default_value="./ipsw")]
    download_path: PathBuf,

    /// Delete old ipsw files when a newer version is available.
    #[structopt(short, long)]
    delete_old_fw: bool,

    /// Download the latest ipsw for all devices.
    #[structopt(short="A", long, conflicts_with("filter-term"), required_unless("filter-term"))]
    download_all: bool,

    /// Filter ipsw files to only device names matching the term.
    #[structopt(short, long, required_unless("download-all"))]
    filter_term: Option<String>,

    /// Directory to create log files in. Will not log if not set.
    #[structopt(short, long)]
    log_path: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let cli: CliOpts = CliOpts::from_args();

    let client = Client::new();

    info!("Beginning program");
    println!("Getting Devices...");

    let devices = client.get_all_devices().await.expect("Cannot hit API!");

    println!("Got {} devices!", devices.len());
    info!("Got {} devices", devices.len());

    Downloader::new(client, devices, cli).begin().await
}