use crate::{Client, CliOpts};
use crate::apiJsonTypes::{Device, FirmwareListing};
use chrono::*;
use std::error::Error;
use log::{info, debug, error};
use std::fs::*;
use std::path::{PathBuf, Path, Component};
use std::sync::atomic::{AtomicBool, Ordering};
use ctrlc::*;
use std::sync::Arc;
use std::io::Write;
use tokio::sync::watch;

pub struct Downloader {
    client: Client,
    devices: Vec<Device>,
    opt: CliOpts,
    start_time: DateTime<chrono::offset::Local>,
    total_done: u32
}

impl Downloader {

    pub fn new(client: Client, devices: Vec<Device>, opt: CliOpts) -> Self {
        Downloader {
            client,
            devices,
            opt,
            start_time: Local::now(),
            total_done: 0
        }
    }

    pub async fn begin(mut self) {

        //If filter is set
        if let Some(filter) = self.opt.filter_term.take() {

            //Download each device that matches filter
            for device in std::mem::take(&mut self.devices).into_iter().filter(|d| d.name.contains(&filter)) {
                let fw = self.client.get_device_firmware(&device).await;

                match fw {
                    Ok(fw) => self.download_firmware(fw).await,
                    Err(why) => Self::report_err(why, &device.name)
                }

                self.after_fw_download();
            }

        } else {
            todo!()
        }

        println!("Finished in {} minutes.", (self.start_time - Local::now()).num_minutes());
        info!("Finished in {} minutes.", (self.start_time - Local::now()).num_minutes())
    }

    async fn download_firmware(&mut self, fw: FirmwareListing) {

        if fw.firmwares.is_empty() {
            println!("{} has no firmware for download", fw.name);
            info!("{} has no firmware for download", fw.name);
            return;
        }

        //Path to file were fw will be
        let file_path = self.opt.download_path.join(fw.name.clone()).join(fw.identifier).with_extension("ipsw");

        //Skip download if file is already downloaded
        if file_path.exists() {
            println!("{} is already downloaded, skipping", fw.name);
            info!("{} is already downloaded", fw.name);
            return;
        }

        //Delete old files if enabled
        if self.opt.delete_old_fw {
            if let Ok(dir) = read_dir(file_path.parent().unwrap()) {
                dir
                    .filter_map(|e| e.ok())
                    .for_each(|e| {
                        match remove_file(e.path()) {
                            Ok(_) => {
                                println!("deleted old file {}", e.file_name().to_str().unwrap());
                                info!("deleted old file {}", e.file_name().to_str().unwrap());
                            }
                            Err(why) => {
                                println!("failed to delete old file {}", e.file_name().to_str().unwrap());
                                error!("failed to delete old file {} because: {}", e.file_name().to_str().unwrap(), why);
                            }
                        }
                    });
            }
        }

        println!("Beginning to download {} {}...", fw.name, fw.firmwares[0].version);
        info!("downloading {} {}", fw.name, fw.firmwares[0].version);

        let (ctrlc_tx,mut ctrlc_rx) = watch::channel(false);

        //Set a bool when ctrlc received
        ctrlc::set_handler(move || {
            println!("ctrlc received, exiting...");
           ctrlc_tx.send(true);
        }).expect("Failed to make the ctrlc handle");

        //Create streams

        //Create dir
        let dir_creation_result = create_dir_all(file_path.parent().unwrap());

        let file_stream = File::create(&file_path);

        if file_stream.is_err() || dir_creation_result.is_err() {
            println!("Could not create file: {} skipping download... {}", file_path.to_str().unwrap(), file_stream.unwrap_err());
            error!("Could not create file: {}", file_path.to_str().unwrap());
            return;
        }
        let mut file_stream = std::io::BufWriter::new(file_stream.unwrap());

        let dl_stream = self.client.download_ipsw(&fw.firmwares[0]).await;

        if dl_stream.is_err() {
            println!("Downloading {} {} errored on Apples API. Skipping download...", fw.name, fw.firmwares[0].identifier);
            error!("Downloading {} {} errored on Apples API", fw.name, fw.firmwares[0].identifier);
            return;
        }
        let mut dl_stream = dl_stream.unwrap();

        use futures::stream::StreamExt; // for `next`

        //Actually download file
        loop {
            //Select over the download and being interrupted
            tokio::select! {
                //Packets for file
                byte = dl_stream.next() => {
                    if let Some(byte) = byte {
                        match byte {
                            Ok(_) => {}
                            Err(_) => {
                                println!("Error writing file: {} skipping download...", file_path.to_str().unwrap());
                                error!("Error writing file: {}", file_path.to_str().unwrap());
                                todo!() //TODO delete file
                            }
                        }

                        match file_stream.write_all(byte.unwrap().as_ref()) {
                            Ok(_) => {}
                            Err(_) => {
                                println!("Error writing file: {} skipping download...", file_path.to_str().unwrap());
                                error!("Error writing file: {}", file_path.to_str().unwrap());
                                todo!()
                            }
                        }
                    } else {
                        break; //Stream done
                    }
                }
                //break if ctrl-c
                _ = ctrlc_rx.changed() => {
                    break;
                    //TODO delete file
                }
            }
        }

    }

    fn report_err(err: impl Error, device: &str) {
        error!("Getting device firmware errored: {}", err);

        println!("Process errored when downloading firmware for {}. Description: {}", device, err)
    }

    /// Performs tasks after a failed or successful download. total done increment, progress bar ect.
    fn after_fw_download(&mut self) {
        todo!()
    }

}