//! Provides a client to access the IPSW.me API.
use bytes::Bytes;
use futures::Stream;
use reqwest::*;

use crate::api_json_types::*;

/// Client to access the ipsw.me API.
pub struct Client {
    internal: reqwest::Client,
}

impl Client {
    pub fn new() -> Self {
        let internal = ClientBuilder::new().build().unwrap();

        Client { internal }
    }

    /// Gets a list of all Apple devices covered by this API.
    ///
    /// # Returns
    /// * Ok(Vec< Value >) - Vec of device json objects.
    /// * Err - The request failed somehow.
    pub async fn get_all_devices(&self) -> Result<Vec<Device>> {
        let response = self.internal.get("https://api.ipsw.me/v4/devices").send().await?;

        Ok(response.json::<Vec<Device>>().await?)
    }

    /// Gets a firmware listing for a given device.
    ///
    /// # Returns
    /// * Ok(FirmwareListing) - info about a device along with its firmware entries. Device name has / and \ replaced with 'z' for use in directories.
    /// * Err - The request failed somehow.
    pub async fn get_device_firmware(&self, device: &Device) -> Result<FirmwareListing> {
        let response = self.internal.get(format!("https://api.ipsw.me/v4/device/{}?type=ipsw", device.identifier)).send().await?;
        let mut firmware = response.json::<FirmwareListing>().await?;

        //Sanitize device name for use in directories
        firmware.name = firmware.name.replace('/', "z");
        firmware.name = firmware.name.replace('\\', "z");

        Ok(firmware)
    }

    /// Begins to download the ipsw file referenced by this firmware.
    ///
    /// # Returns
    /// * Ok(stream, dl_size) - The ipsw file being downloaded as an async byte stream, and the length in bytes of that stream.
    /// * Err - Errored when hitting Apples API. This can happen for old ipsw files.
    pub async fn download_ipsw(&self, fw: &Firmware) -> Result<(impl Stream<Item = Result<Bytes>>, u64)> {
        let response = self.internal.get(format!("https://api.ipsw.me/v4/ipsw/download/{}/{}", fw.identifier, fw.buildid)).send().await?;

        //TODO check for non-200 code

        let len = response.content_length().unwrap();
        Ok((response.bytes_stream(), len))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn get_all_devices_works() {
        let client = Client::new();

        let response = client.get_all_devices().await.expect("Couldnt hit API!");

        assert!(response.len() > 1, "retrieved no devices");
        assert_ne!(response[0].name, String::new(), "device names are empty");

        println!("{:?}", response)
    }

    #[tokio::test]
    async fn get_device_firmware_works() {
        let client = Client::new();

        let response = client.get_all_devices().await.expect("Couldnt hit API!");

        let response = client.get_device_firmware(&response[0]).await.expect("Couldnt hit API!");

        assert!(!response.firmwares.is_empty());

        println!("{:?}", response)
    }



}
