pub mod runner;
pub mod providers;

use std::sync::OnceLock;
use reqwest::Client;

pub static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

pub fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client")
    })
}
