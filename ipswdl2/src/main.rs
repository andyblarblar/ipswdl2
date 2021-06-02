
use std::fs::File;
use std::path::PathBuf;

use log::{info, LevelFilter};
use simplelog::{Config, WriteLogger};
use structopt::*;

use crate::client::Client;
use crate::downloader::Downloader;

mod client;
mod api_json_types;
mod downloader;

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
    #[structopt(short="A", long, conflicts_with("filter-term"), required_unless("filter-term"), required_unless("list-device-names"))]
    #[allow(dead_code)]
    download_all: bool, //Never used, but needed to avoid CLI from running without user input

    /// Filter ipsw files to only device names matching the term.
    #[structopt(short, long, required_unless("download-all"), required_unless("list-device-names"))]
    filter_term: Option<String>,

    /// Directory to create log files in. Will not log if not set.
    #[structopt(short, long)]
    log_path: Option<PathBuf>,

    /// List all device names that could be downloaded. Should only be used by itself.
    #[structopt(short="L", long, conflicts_with("filter-term"), conflicts_with("download-all"))]
    list_device_names: bool
}

#[tokio::main]
async fn main() {
    let cli: CliOpts = CliOpts::from_args();

    //Init logging if option is passed
    if let Some(path) = &cli.log_path {
        WriteLogger::init(LevelFilter::Debug, Config::default(), File::create(path).expect("log-path is in invalid file!")).unwrap();
    }

    let client = Client::new();

    println!("Getting Devices...");

    let devices = client.get_all_devices().await.expect("Cannot hit API!");

    //List devices if flag is set
    if cli.list_device_names {
        for device in &devices {
            println!("{}", device.name);
        }
        return
    }

    println!("Got {} devices!", devices.len());
    info!("Got {} devices", devices.len());

    Downloader::new(client, devices, cli).begin().await
}