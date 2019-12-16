use proxmox_api_macro::api;

use failure::Error;
use serde_json::Value;

#[api(
    input: {
        properties: {
            message: {
                description: "The message to print",
            }
        }
    }
)]
/// Print the given message.
pub fn hello(message: String) -> Result<(), Error> {
    println!("Hello there. {}", message);
    Ok(())
}

#[api(
    input: {
        properties: {
            num: {
                description: "The version to upgrade to",
            },
        },
    },
)]
/// Return the number...
pub async fn number(num: u32) -> Result<u32, Error> {
    Ok(num)
}
