//! Contains types for binding to JSON API responses.
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Device {
    pub name: String,
    pub identifier: String,
    pub platform: String,
    pub cpid: u32,
    pub bdid: u32,
}

//only used in firmware listing
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Firmware {
    pub identifier: String,
    pub version: String,
    pub buildid: String,
    pub sha1sum: String,
    pub md5sum: String,
    pub filesize: u64,
    pub url: String,
    pub uploaddate: DateTime<Utc>
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FirmwareListing {
    pub name: String,
    pub identifier: String,
    pub platform: String,
    pub boardconfig: String,
    pub cpid: u32,
    pub bdid: u32,
    pub firmwares: Vec<Firmware> //Chrono ordered by api
}



