use anyhow::{bail, format_err, Error};

use lazy_static::lazy_static;
use regex::Regex;
use serde_json::json;

use proxmox_http::{uri::json_object_to_query, HttpClient};

use crate::{
    subscription_info::{md5sum, SHARED_KEY_DATA},
    SubscriptionInfo, SubscriptionStatus,
};

lazy_static! {
    static ref ATTR_RE: Regex = Regex::new(r"<([^>]+)>([^<]+)</[^>]+>").unwrap();
}

const SHOP_URI: &str = "https://shop.proxmox.com/modules/servers/licensing/verify.php";

/// (Re)-register a subscription key with the WHMCS server.
fn register_subscription<C: HttpClient<String, String>>(
    key: &str,
    server_id: &str,
    checktime: i64,
    client: C,
) -> Result<(String, String), Error> {
    // WHCMS sample code feeds the key into this, but it's just a challenge, so keep it simple
    let rand = hex::encode(&proxmox_sys::linux::random_data(16)?);
    let challenge = format!("{}{}", checktime, rand);

    let params = json!({
        "licensekey": key,
        "dir": server_id,
        "domain": "www.proxmox.com",
        "ip": "localhost",
        "check_token": challenge,
    });

    let query = json_object_to_query(params)?;
    let response = client.post(
        SHOP_URI,
        Some(query),
        Some("application/x-www-form-urlencoded"),
        None,
    )?;
    let body = response.into_body();

    Ok((body, challenge))
}

fn parse_status(value: &str) -> SubscriptionStatus {
    match value.to_lowercase().as_str() {
        "active" => SubscriptionStatus::Active,
        "new" => SubscriptionStatus::New,
        "notfound" => SubscriptionStatus::NotFound,
        "invalid" => SubscriptionStatus::Invalid,
        "expired" => SubscriptionStatus::Expired,
        _ => SubscriptionStatus::Invalid,
    }
}

fn parse_register_response(
    body: &str,
    key: String,
    server_id: String,
    checktime: i64,
    challenge: &str,
    product_url: String,
) -> Result<SubscriptionInfo, Error> {
    let mut info = SubscriptionInfo {
        key: Some(key),
        status: SubscriptionStatus::NotFound,
        checktime: Some(checktime),
        url: Some(product_url),
        ..Default::default()
    };
    let mut md5hash = String::new();
    let is_server_id = |id: &&str| *id == server_id;

    for caps in ATTR_RE.captures_iter(body) {
        let (key, value) = (&caps[1], &caps[2]);
        match key {
            "status" => info.status = parse_status(value),
            "productname" => info.productname = Some(value.into()),
            "regdate" => info.regdate = Some(value.into()),
            "nextduedate" => info.nextduedate = Some(value.into()),
            "message" if value == "Directory Invalid" => {
                info.message = Some("Invalid Server ID".into())
            }
            "message" => info.message = Some(value.into()),
            "validdirectory" => {
                if value.split(',').find(is_server_id) == None {
                    bail!("Server ID does not match");
                }
                info.serverid = Some(server_id.to_owned());
            }
            "md5hash" => md5hash = value.to_owned(),
            _ => (),
        }
    }

    if let SubscriptionStatus::Active = info.status {
        let response_raw = format!("{}{}", SHARED_KEY_DATA, challenge);
        let expected = hex::encode(md5sum(response_raw.as_bytes())?);

        if expected != md5hash {
            bail!(
                "Subscription API challenge failed, expected {} != got {}",
                expected,
                md5hash
            );
        }
    }
    Ok(info)
}

#[test]
fn test_parse_register_response() -> Result<(), Error> {
    let response = r#"
<status>Active</status>
<companyname>Proxmox</companyname>
<serviceid>41108</serviceid>
<productid>71</productid>
<productname>Proxmox Backup Server Test Subscription -1 year</productname>
<regdate>2020-09-19 00:00:00</regdate>
<nextduedate>2021-09-19</nextduedate>
<billingcycle>Annually</billingcycle>
<validdomain>proxmox.com,www.proxmox.com</validdomain>
<validdirectory>830000000123456789ABCDEF00000042</validdirectory>
<customfields>Notes=Test Key!</customfields>
<addons></addons>
<md5hash>969f4df84fe157ee4f5a2f71950ad154</md5hash>
"#;
    let key = "pbst-123456789a".to_string();
    let server_id = "830000000123456789ABCDEF00000042".to_string();
    let checktime = 1600000000;
    let salt = "cf44486bddb6ad0145732642c45b2957";

    let info = parse_register_response(
        response,
        key.to_owned(),
        server_id.to_owned(),
        checktime,
        salt,
        "https://www.proxmox.com/en/proxmox-backup-server/pricing".to_string(),
    )?;

    assert_eq!(
        info,
        SubscriptionInfo {
            key: Some(key),
            serverid: Some(server_id),
            status: SubscriptionStatus::Active,
            checktime: Some(checktime),
            url: Some("https://www.proxmox.com/en/proxmox-backup-server/pricing".into()),
            message: None,
            nextduedate: Some("2021-09-19".into()),
            regdate: Some("2020-09-19 00:00:00".into()),
            productname: Some("Proxmox Backup Server Test Subscription -1 year".into()),
            signature: None,
        }
    );
    Ok(())
}

/// Queries the WHMCS server to register/update the subscription key information, parsing the
/// response into a [SubscriptionInfo].
pub fn check_subscription<C: HttpClient<String, String>>(
    key: String,
    server_id: String,
    product_url: String,
    http_client: C,
) -> Result<SubscriptionInfo, Error> {
    let now = proxmox_time::epoch_i64();

    let (response, challenge) = register_subscription(&key, &server_id, now, http_client)
        .map_err(|err| format_err!("Error checking subscription: {}", err))?;

    parse_register_response(&response, key, server_id, now, &challenge, product_url)
        .map_err(|err| format_err!("Error parsing subscription check response: {}", err))
}
