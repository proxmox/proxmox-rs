//! Helpers for request authentication using AWS signature version 4

use anyhow::{bail, Error};
use hyper::Request;
use openssl::hash::MessageDigest;
use openssl::pkey::{PKey, Private};
use openssl::sha::sha256;
use openssl::sign::Signer;
use url::Url;

use proxmox_http::Body;

use super::client::S3ClientOptions;

pub(crate) const AWS_SIGN_V4_DATETIME_FORMAT: &str = "%Y%m%dT%H%M%SZ";

const AWS_SIGN_V4_DATE_FORMAT: &str = "%Y%m%d";
const AWS_SIGN_V4_SERVICE_S3: &str = "s3";
const AWS_SIGN_V4_REQUEST_POSTFIX: &str = "aws4_request";

/// Generate signature for S3 request authentication using AWS signature version 4.
/// See: https://docs.aws.amazon.com/AmazonS3/latest/API/sig-v4-authenticating-requests.html
pub(crate) fn aws_sign_v4_signature(
    request: &Request<Body>,
    options: &S3ClientOptions,
    epoch: i64,
    payload_digest: &str,
) -> Result<String, Error> {
    // Include all headers in signature calculation since the reference docs note:
    // "For the purpose of calculating an authorization signature, only the 'host' and any 'x-amz-*'
    // headers are required. however, in order to prevent data tampering, you should consider
    // including all the headers in the signature calculation."
    // See https://docs.aws.amazon.com/AmazonS3/latest/API/sig-v4-header-based-auth.html
    let mut canonical_headers = Vec::new();
    let mut signed_headers = Vec::new();
    for (key, value) in request.headers() {
        canonical_headers.push(format!(
            "{}:{}",
            // Header name has to be lower case, key.as_str() does guarantee that, see
            // https://docs.rs/http/0.2.0/http/header/struct.HeaderName.html
            key.as_str(),
            // No need to trim since `HeaderValue` only allows visible UTF8 chars, see
            // https://docs.rs/http/0.2.0/http/header/struct.HeaderValue.html
            value.to_str()?,
        ));
        signed_headers.push(key.as_str());
    }
    canonical_headers.sort();
    signed_headers.sort();
    let signed_headers_string = signed_headers.join(";");

    let mut canonical_queries = Url::parse(&request.uri().to_string())?
        .query_pairs()
        .map(|(key, value)| {
            format!(
                "{}={}",
                aws_sign_v4_uri_encode(&key, false),
                aws_sign_v4_uri_encode(&value, false),
            )
        })
        .collect::<Vec<String>>();
    canonical_queries.sort();

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n\n{}\n{}",
        request.method().as_str(),
        request.uri().path(),
        canonical_queries.join("&"),
        canonical_headers.join("\n"),
        signed_headers_string,
        payload_digest,
    );

    let date = proxmox_time::strftime_utc(AWS_SIGN_V4_DATE_FORMAT, epoch)?;
    let datetime = proxmox_time::strftime_utc(AWS_SIGN_V4_DATETIME_FORMAT, epoch)?;

    let credential_scope = format!(
        "{date}/{}/{AWS_SIGN_V4_SERVICE_S3}/{AWS_SIGN_V4_REQUEST_POSTFIX}",
        options.region,
    );
    let canonical_request_hash = hex::encode(sha256(canonical_request.as_bytes()));
    let string_to_sign =
        format!("AWS4-HMAC-SHA256\n{datetime}\n{credential_scope}\n{canonical_request_hash}");

    let date_sign_key = PKey::hmac(format!("AWS4{}", options.secret_key).as_bytes())?;
    let date_tag = hmac_sha256(&date_sign_key, date.as_bytes())?;

    let region_sign_key = PKey::hmac(&date_tag)?;
    let region_tag = hmac_sha256(&region_sign_key, options.region.as_bytes())?;

    let service_sign_key = PKey::hmac(&region_tag)?;
    let service_tag = hmac_sha256(&service_sign_key, AWS_SIGN_V4_SERVICE_S3.as_bytes())?;

    let signing_key = PKey::hmac(&service_tag)?;
    let signing_tag = hmac_sha256(&signing_key, AWS_SIGN_V4_REQUEST_POSTFIX.as_bytes())?;

    let signature_key = PKey::hmac(&signing_tag)?;
    let signature = hmac_sha256(&signature_key, string_to_sign.as_bytes())?;
    let signature = hex::encode(&signature);

    Ok(format!(
        "AWS4-HMAC-SHA256 Credential={}/{credential_scope},SignedHeaders={signed_headers_string},Signature={signature}",
        options.access_key,
    ))
}
// Custom `uri_encode` implementation as recommended by AWS docs, since possible implementation
// incompatibility with uri encoding libraries.
// See: https://docs.aws.amazon.com/AmazonS3/latest/API/sigv4-query-string-auth.html
pub(crate) fn aws_sign_v4_uri_encode(input: &str, is_object_key_name: bool) -> String {
    // Assume up to  2 bytes per char max in output
    let mut accumulator = String::with_capacity(2 * input.len());

    input.chars().for_each(|char| {
        match char {
            // Unreserved characters, do not uri encode these bytes
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '.' | '_' | '~' => accumulator.push(char),
            // Space character is reserved, must be encoded as '%20', not '+'
            ' ' => accumulator.push_str("%20"),
            // Encode the forward slash character, '/', everywhere except in the object key name
            '/' if !is_object_key_name => accumulator.push_str("%2F"),
            '/' if is_object_key_name => accumulator.push(char),
            // URI encoded byte is formed by a '%' and the two-digit hexadecimal value of the byte
            // Letters in the hexadecimal value must be uppercase
            _ => {
                for byte in char.to_string().as_bytes() {
                    accumulator.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    });

    accumulator
}

// Helper for hmac sha256 calculation
fn hmac_sha256(key: &PKey<Private>, data: &[u8]) -> Result<Vec<u8>, Error> {
    let mut signer = Signer::new(MessageDigest::sha256(), key)?;
    signer.update(data)?;
    let hmac = signer.sign_to_vec()?;
    Ok(hmac)
}

/// Custom `uri_decode` implementation
pub fn uri_decode(input: &str) -> Result<String, Error> {
    // Require full capacity if no characters are encoded, less otherwise
    let mut accumulator = String::with_capacity(input.len());
    let mut subslices_iter = input.split('%');
    // First item present also when empty, nevertheless fallback to empty default
    accumulator.push_str(subslices_iter.next().unwrap_or(""));

    for subslice in subslices_iter {
        if let Some((hex_digits, utf8_rest)) = subslice.as_bytes().split_at_checked(2) {
            let mut ascii_code = 0u8;
            for (pos, digit) in hex_digits.iter().enumerate().take(2) {
                let val = match digit {
                    b'0'..=b'9' => digit - b'0',
                    b'A'..=b'F' => digit - b'A' + 10,
                    b'a'..=b'f' => digit - b'a' + 10,
                    _ => bail!("unexpected hex digit at %{subslice}"),
                };
                // Shift first diigts value to be upper byte half
                ascii_code += val << (4 * ((pos + 1) % 2));
            }
            accumulator.push(ascii_code as char);
            // Started from valid utf-8 without modification
            let rest = unsafe { std::str::from_utf8_unchecked(utf8_rest) };
            accumulator.push_str(rest);
        } else {
            bail!("failed to decode string at subslice %{subslice}");
        }
    }

    Ok(accumulator)
}

#[test]
fn test_aws_sign_v4_uri_encode() {
    assert_eq!(aws_sign_v4_uri_encode("AZaz09-._~", false), "AZaz09-._~");
    assert_eq!(aws_sign_v4_uri_encode("a b", false), "a%20b");
    assert_eq!(
        aws_sign_v4_uri_encode("/path/to/object", false),
        "%2Fpath%2Fto%2Fobject"
    );
    assert_eq!(
        aws_sign_v4_uri_encode("/path/to/object", true),
        "/path/to/object"
    );
    assert_eq!(
        aws_sign_v4_uri_encode(" !\"#$%&'()*+,:;=?@[]", false),
        "%20%21%22%23%24%25%26%27%28%29%2A%2B%2C%3A%3B%3D%3F%40%5B%5D"
    );
    assert_eq!(aws_sign_v4_uri_encode("", false), "");
}

#[test]
fn test_uri_decode() {
    assert_eq!(uri_decode("a%20b%2FC").unwrap(), "a b/C");
    assert_eq!(uri_decode("a%20b%2fc").unwrap(), "a b/c");
    assert_eq!(uri_decode("simple-string").unwrap(), "simple-string");
    assert_eq!(uri_decode("").unwrap(), "");
    assert!(
        uri_decode("test%").is_err(),
        "Incomplete escape sequence at end"
    );
    assert!(
        uri_decode("test%F").is_err(),
        "Incomplete two-digit escape sequence"
    );
    assert!(uri_decode("test%GZ").is_err(), "Invalid hex digit");
}
