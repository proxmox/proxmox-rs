// Execute via `cargo run --example s3_client --features impl` in `proxmox` main repo folder

#[cfg(not(feature = "impl"))]
fn main() {
    // intentionally left empty
}

#[cfg(feature = "impl")]
use proxmox_s3_client::{S3Client, S3ClientOptions, S3ObjectKey, S3PathPrefix};

#[cfg(feature = "impl")]
fn main() -> Result<(), anyhow::Error> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run())
}

#[cfg(feature = "impl")]
async fn run() -> Result<(), anyhow::Error> {
    // Configure the client via the client options
    let options = S3ClientOptions {
        // Must be resolvable, e.g. the Ceph RADOS gateway.
        // Allows to use {{bucket}} or {{region}} template pattern for ease of configuration.
        // In this example, the final authority is `https://testbucket.s3.pve-c1.local:7480/`.
        endpoint: "{{bucket}}.s3.pve-c1.local".to_string(),
        // Must match the port the api is listening on
        port: Some(7480),
        // Name of the bucket to be used
        bucket: "testbucket".to_string(),
        common_prefix: "teststore".to_string(),
        path_style: false,
        access_key: "<your-access-key>".to_string(),
        secret_key: "<your-secret-key>".to_string(),
        region: "us-west-1".to_string(),
        // Only required for self signed certificates, can be obtained by, e.g.
        // `openssl s_client -connect testbucket.s3.pve-c1.local:7480 < /dev/null | openssl x509 -fingerprint -sha256 -noout`
        fingerprint: Some("<s3-api-fingerprint>".to_string()),
        put_rate_limit: None,
    };

    // Creating a client instance and connect to api endpoint
    let s3_client = S3Client::new(options)?;

    // Check if the bucket can be accessed
    s3_client.head_bucket().await?;

    let rel_object_key = S3ObjectKey::try_from("object.txt")?;
    let body = proxmox_http::Body::empty();
    let replace_existing_key = true;
    let _response = s3_client
        .put_object(rel_object_key, body, replace_existing_key)
        .await?;

    // List object, limiting to ones matching the given prefix. Since the api limits the response
    // to 1000 entries, the following contents might be fetched using a continuation token, being
    // part of the previouis response.
    let prefix = S3PathPrefix::Some("/teststore/".to_string());
    let continuation_token = None;
    let _response = s3_client
        .list_objects_v2(&prefix, continuation_token)
        .await?;

    // Delete a single object
    let rel_object_key = S3ObjectKey::try_from("object.txt")?;
    let _response = s3_client.delete_object(rel_object_key).await?;
    Ok(())
}
