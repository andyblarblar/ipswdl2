use std::error::Error;
use std::fs::*;
use std::io::{Write};

use chrono::*;
use indicatif::{ProgressStyle};
use log::{debug, error, info};
use tokio::sync::watch;
use tokio::sync::watch::Receiver;

use crate::{Client, CliOpts};
use crate::api_json_types::{Device, FirmwareListing};

pub struct Downloader {
    /// Client to access IPSW API.
    client: Client,
    /// List of all devices to download.
    devices: Vec<Device>,
    /// Options passed to the command line.
    opt: CliOpts,
    /// Time the Downloader object was made.
    start_time: DateTime<chrono::offset::Local>,
    /// Devices processed thus far.
    total_done: u32,
    /// Devices to be processed.
    total_todo: u32,
    /// Async channel that receives `true` when ctrlc is passed.
    ctrlc_received: Receiver<bool>,
    /// `true` if program should abort when the next download starts.
    /// Currently only used for the ctlc handle, but could also be used to make an error fatal.
    kill_program: bool
}

/// True if there is a downloader instance currently in use in any scope.
static mut DOWNLOADER_CREATED: bool = false;

impl Downloader {

    /// Creates a new downloader.
    pub fn new(client: Client, devices: Vec<Device>, opt: CliOpts) -> Self {

        //Ensure downloader is singleton
        unsafe {
            if DOWNLOADER_CREATED {
                panic!("Created two Downloader instances! This would cause an error when binding ctrlc. Please use a lazy_static instead.");
            }
            DOWNLOADER_CREATED = true;
        }

        //bind ctrlc to a channel
        let (ctrlc_tx, ctrlc_rx) = watch::channel(false);
        ctrlc::set_handler(move || {
            println!("ctrlc received, exiting...");
            error!("Killed by ctrlc");
            ctrlc_tx.send(true).unwrap();
        }).expect("Failed to make the ctrlc handle");

        Downloader {
            client,
            total_todo: devices.len() as u32,
            devices,
            opt,
            start_time: Local::now(),
            total_done: 0,
            ctrlc_received: ctrlc_rx,
            kill_program: false
        }
    }

    pub async fn begin(mut self) {
        //If filter is set
        if let Some(filter) = self.opt.filter_term.take() {
            debug!("using filter: {}", filter);

            //Download each device that matches filter
            for device in std::mem::take(&mut self.devices)
                .into_iter()
                .filter(|d| d.name.contains(&filter))
            {
                //Return early if told to die
                if self.kill_program {
                    return
                }

                let fw = self.client.get_device_firmware(&device).await;

                match fw {
                    Ok(fw) => self.download_firmware(fw).await,
                    Err(why) => Self::report_err(why, &device.name),
                }

                self.after_fw_download(&device);
            }
        } else {
            //Download all
            for device in std::mem::take(&mut self.devices)
            {
                //Return early if told to die
                if self.kill_program {
                    return
                }

                let fw = self.client.get_device_firmware(&device).await;

                match fw {
                    Ok(fw) => self.download_firmware(fw).await,
                    Err(why) => Self::report_err(why, &device.name),
                }

                self.after_fw_download(&device);
            }
        }

        println!(
            "Finished in {} minutes.",
            (self.start_time - Local::now()).num_minutes()
        );
        info!(
            "Finished in {} minutes.",
            (self.start_time - Local::now()).num_minutes()
        )
    }

    /// Downloads the newest firmware contained in the passed firmware listing.
    ///
    /// details
    /// -------
    ///
    /// The download will begin in an OS temp file, and then copied to the final directory indicated by the CLI options.
    /// All errors occurred in the download process will be handled by it. Should the ctrl-c signal be received,
    /// the function will abort unless copying the temp file to the final destination, ensuring only valid files are
    /// left in the destination folder.
    async fn download_firmware(&mut self, fw: FirmwareListing) {
        if fw.firmwares.is_empty() {
            println!("{} has no firmware for download", fw.name);
            info!("{} has no firmware for download", fw.name);
            return;
        }

        //Path to file were fw will be
        let file_path = self
            .opt
            .download_path
            .join(fw.name.clone())
            .join(fw.identifier)
            .with_extension("ipsw");

        //Skip download if file is already downloaded
        if file_path.exists() {
            println!("{} is already downloaded, skipping", fw.name);
            info!("{} is already downloaded", fw.name);
            return;
        }

        //Delete old files if enabled
        if self.opt.delete_old_fw {
            if let Ok(dir) = read_dir(file_path.parent().unwrap()) {
                dir.filter_map(|e| e.ok())
                    .for_each(|e| match remove_file(e.path()) {
                        Ok(_) => {
                            println!("deleted old file {}", e.file_name().to_str().unwrap());
                            info!("deleted old file {}", e.file_name().to_str().unwrap());
                        }
                        Err(why) => {
                            println!(
                                "failed to delete old file {}",
                                e.file_name().to_str().unwrap()
                            );
                            error!(
                                "failed to delete old file {} because: {}",
                                e.file_name().to_str().unwrap(),
                                why
                            );
                        }
                    });
            }
        }

        println!(
            "Beginning to download {} {}...",
            fw.name, fw.firmwares[0].version
        );
        info!("downloading {} {}", fw.name, fw.firmwares[0].version);

        //Create streams

        //Temp file to dl to first. This avoids leaving a bad file if program is killed
        let temp_file_stream = tempfile::NamedTempFile::new().unwrap();
        //Copy file handle for reading later
        let temp_file_read = temp_file_stream.reopen().unwrap();
        let mut temp_file_stream = std::io::BufWriter::new(temp_file_stream);

        //Get the stream to download
        let dl_stream = self.client.download_ipsw(&fw.firmwares[0]).await;
        if dl_stream.is_err() {
            println!(
                "Downloading {} {} errored on Apples API. Skipping download...",
                fw.name, fw.firmwares[0].identifier
            );
            error!(
                "Downloading {} {} errored on Apples API",
                fw.name, fw.firmwares[0].identifier
            );
            return;
        }
        let (mut dl_stream, dl_size) = dl_stream.unwrap();

        //Set up progress bar
        let download_progress_bar = indicatif::ProgressBar::new(dl_size);
        download_progress_bar.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"));

        use futures::stream::StreamExt; // for `next`

        //Actually download file
        loop {
            //Select over the download and being interrupted
            tokio::select! {
                //Packets for file
                byte = dl_stream.next() => {
                    if let Some(byte) = byte {
                        //Catch errors
                        match byte {
                            Ok(_) => {}
                            Err(_) => {
                                println!("Error writing file: {} skipping download...", file_path.to_str().unwrap());
                                error!("Error writing file: {}", file_path.to_str().unwrap());
                            }
                        }

                        //Update dl bar
                        let byte = byte.unwrap();
                        download_progress_bar.inc(byte.len() as u64);

                        match temp_file_stream.write_all(byte.as_ref()) {
                            Ok(_) => {}
                            Err(_) => {
                                println!("Error writing file: {} skipping download...", file_path.to_str().unwrap());
                                error!("Error writing file: {}", file_path.to_str().unwrap());
                            }
                        }
                    } else { //Stream done

                        //Create final file now
                        let dir_creation_result = create_dir_all(file_path.parent().unwrap());
                        let file_stream = File::create(&file_path);

                        if file_stream.is_err() || dir_creation_result.is_err() {
                            println!(
                                "Could not create file: {} skipping download... {}",
                                file_path.to_str().unwrap(),
                                file_stream.unwrap_err()
                            );
                            error!("Could not create file: {}", file_path.to_str().unwrap());
                            return;
                        }
                        //The file stream to the final file
                        let file_stream = file_stream.unwrap();
                        let mut end_file_stream = std::io::BufWriter::new(file_stream);

                        //Copy the downloaded file to the final path now that the dl is done.
                        debug!("Copying from temp file to end file");
                        match std::io::copy(&mut std::io::BufReader::new(temp_file_read), &mut end_file_stream) {
                            Err(why) => {
                                println!(
                                    "Could not create file: {} skipping download... {}",
                                    file_path.to_str().unwrap(),
                                    why
                                );
                                error!("Could not copy temp to file: {} err: {}", file_path.to_str().unwrap(), why);
                                return;
                            },
                            Ok(bytes) if bytes == 0 => log::warn!("Didn't copy any bytes to final file!"),
                            Ok(bytes) => debug!("Copied {} bytes to final file", bytes)
                        }

                        break;
                    }
                }

                //break if ctrl-c passed
                _ = self.ctrlc_received.changed() => {
                    self.kill_program = true;
                    break;
                }
            }
        }
    }

    /// Reports a device firmware download error.
    fn report_err(err: impl Error, device: &str) {
        error!("Getting device firmware errored: {}", err);

        println!(
            "Process errored when downloading firmware for {}. Description: {}",
            device, err
        )
    }

    /// Performs tasks after a failed or successful download. total done increment, progress bar ect.
    fn after_fw_download(&mut self, device: &Device) {
        self.total_done += 1;
        println!("Ended work on: {} ({}/{})", device.name, self.total_done, self.total_todo);
        //TODO add color
    }
}

impl Drop for Downloader {
    fn drop(&mut self) {
        //Reset instance count, as we no longer exist.
        unsafe {
            DOWNLOADER_CREATED = false;
        }
    }
}
