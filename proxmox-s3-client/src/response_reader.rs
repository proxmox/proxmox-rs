use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Error};
use http_body_util::BodyExt;
use hyper::body::{Bytes, Incoming};
use hyper::header::HeaderName;
use hyper::http::header;
use hyper::http::StatusCode;
use hyper::{HeaderMap, Response};
use serde::Deserialize;

use crate::S3ObjectKey;
use crate::{HttpDate, LastModifiedTimestamp};

pub(crate) struct ResponseReader {
    response: Response<Incoming>,
}

#[derive(Debug)]
/// Subset of the list object v2 response including some header values
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_CopyObject.html#API_CopyObject_ResponseSyntax
pub struct ListObjectsV2Response {
    pub date: HttpDate,
    pub name: String,
    pub prefix: String,
    pub key_count: u64,
    pub max_keys: u64,
    pub is_truncated: bool,
    pub continuation_token: Option<String>,
    pub next_continuation_token: Option<String>,
    pub contents: Vec<ListObjectsV2Contents>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
/// Subset of items used to deserialize a list objects v2 respsonse
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_CopyObject.html#API_CopyObject_ResponseSyntax
struct ListObjectsV2ResponseBody {
    pub name: String,
    pub prefix: String,
    pub key_count: u64,
    pub max_keys: u64,
    pub is_truncated: bool,
    pub continuation_token: Option<String>,
    pub next_continuation_token: Option<String>,
    pub contents: Option<Vec<ListObjectsV2Contents>>,
}

impl ListObjectsV2ResponseBody {
    fn with_date(self, date: HttpDate) -> ListObjectsV2Response {
        ListObjectsV2Response {
            date,
            name: self.name,
            prefix: self.prefix,
            key_count: self.key_count,
            max_keys: self.max_keys,
            is_truncated: self.is_truncated,
            continuation_token: self.continuation_token,
            next_continuation_token: self.next_continuation_token,
            contents: self.contents.unwrap_or_default(),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
/// Subset used to deserialize the contents of a list objects v2 respsonse
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_CopyObject.html#API_CopyObject_ResponseSyntax
pub struct ListObjectsV2Contents {
    pub key: S3ObjectKey,
    pub last_modified: LastModifiedTimestamp,
    pub e_tag: String,
    pub size: u64,
    pub storage_class: String,
}

#[derive(Debug)]
/// Subset of the head object response (headers only, there is no body)
/// See https://docs.aws.amazon.com/AmazonS3/latest/API/API_HeadObject.html#API_HeadObject_ResponseSyntax
pub struct HeadObjectResponse {
    pub content_length: u64,
    pub content_type: String,
    pub date: HttpDate,
    pub e_tag: String,
    pub last_modified: HttpDate,
}

/// Subset of the get object response including some headers
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_GetObject.html#API_GetObject_ResponseSyntax
pub struct GetObjectResponse {
    pub content_length: u64,
    pub content_type: String,
    pub date: HttpDate,
    pub e_tag: String,
    pub last_modified: HttpDate,
    pub content: Incoming,
}

/// Subset of the put object response
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_PutObject.html#API_PutObject_ResponseSyntax
#[derive(Debug)]
pub enum PutObjectResponse {
    NeedsRetry,
    PreconditionFailed,
    Success(String),
}

/// Subset of the delete objects response
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_DeleteObjects.html#API_DeleteObjects_ResponseElements
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "PascalCase")]
pub struct DeleteObjectsResponse {
    pub deleted: Option<Vec<DeletedObject>>,
    pub error: Option<Vec<DeleteObjectError>>,
}

/// Subset used to deserialize the deleted objects of a delete objects v2 respsonse
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_DeletedObject.html
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct DeletedObject {
    pub delete_marker: Option<bool>,
    pub delete_marker_version_id: Option<String>,
    pub key: Option<S3ObjectKey>,
    pub version_id: Option<String>,
}

/// Subset used to deserialize the deleted object errors of a delete objects v2 respsonse
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_Error.html
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct DeleteObjectError {
    pub code: Option<String>,
    pub key: Option<S3ObjectKey>,
    pub message: Option<String>,
    pub version_id: Option<String>,
}

#[derive(Debug)]
/// Subset used to deserialize the copy object response
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_CopyObject.html#API_CopyObject_ResponseSyntax
pub struct CopyObjectResponse {
    pub copy_object_result: CopyObjectResult,
    pub x_amz_version_id: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
/// Subset used to deserialize the copy object result of a copy object respsonse
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_CopyObject.html#API_CopyObject_ResponseSyntax
pub struct CopyObjectResult {
    pub e_tag: String,
    pub last_modified: LastModifiedTimestamp,
}

/// Subset of the list buckets response
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListBuckets.html#API_ListBuckets_ResponseElements
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct ListBucketsResponse {
    pub buckets: Vec<Bucket>,
}
/// Subset of the list buckets response
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListBuckets.html#API_ListBuckets_ResponseElements
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct ListAllMyBucketsResult {
    pub buckets: Option<Buckets>,
}

/// Subset used to deserialize the list buckets response
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Buckets {
    bucket: Vec<Bucket>,
}

/// Subset used to deserialize the list buckets response
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Bucket {
    pub name: String,
    pub bucket_arn: Option<String>,
    pub bucket_region: Option<String>,
    pub creation_date: LastModifiedTimestamp,
}

impl ResponseReader {
    pub(crate) fn new(response: Response<Incoming>) -> Self {
        Self { response }
    }

    pub(crate) async fn list_objects_v2_response(self) -> Result<ListObjectsV2Response, Error> {
        let (parts, body) = self.response.into_parts();
        let body = body.collect().await?.to_bytes();

        match parts.status {
            StatusCode::OK => (),
            StatusCode::NOT_FOUND => bail!("bucket does not exist"),
            status_code => {
                Self::log_error_response_utf8(body);
                bail!("unexpected status code {status_code}")
            }
        }

        let body = String::from_utf8(body.to_vec())?;

        let date: HttpDate = Self::parse_header(header::DATE, &parts.headers)?;

        let response: ListObjectsV2ResponseBody =
            serde_xml_rs::from_str(&body).context("failed to parse response body")?;

        Ok(response.with_date(date))
    }

    pub(crate) async fn head_object_response(self) -> Result<Option<HeadObjectResponse>, Error> {
        let (parts, body) = self.response.into_parts();
        let body = body.collect().await?.to_bytes();

        match parts.status {
            StatusCode::OK => (),
            StatusCode::NOT_FOUND => return Ok(None),
            status_code => {
                Self::log_error_response_utf8(body);
                bail!("unexpected status code {status_code}")
            }
        }
        if !body.is_empty() {
            bail!("got unexpected non-empty response body");
        }

        let content_length: u64 = Self::parse_header(header::CONTENT_LENGTH, &parts.headers)?;
        let content_type = Self::parse_header(header::CONTENT_TYPE, &parts.headers)?;
        let e_tag = Self::parse_header(header::ETAG, &parts.headers)?;
        let date = Self::parse_header(header::DATE, &parts.headers)?;
        let last_modified = Self::parse_header(header::LAST_MODIFIED, &parts.headers)?;

        Ok(Some(HeadObjectResponse {
            content_length,
            content_type,
            date,
            e_tag,
            last_modified,
        }))
    }

    pub(crate) async fn get_object_response(self) -> Result<Option<GetObjectResponse>, Error> {
        let (parts, content) = self.response.into_parts();

        match parts.status {
            StatusCode::OK => (),
            StatusCode::NOT_FOUND => return Ok(None),
            StatusCode::FORBIDDEN => bail!("object is archived and inaccessible until restored"),
            status_code => {
                let body = content.collect().await?.to_bytes();
                Self::log_error_response_utf8(body);
                bail!("unexpected status code {status_code}")
            }
        }

        let content_length: u64 = Self::parse_header(header::CONTENT_LENGTH, &parts.headers)?;
        let content_type = Self::parse_header(header::CONTENT_TYPE, &parts.headers)?;
        let e_tag = Self::parse_header(header::ETAG, &parts.headers)?;
        let date = Self::parse_header(header::DATE, &parts.headers)?;
        let last_modified = Self::parse_header(header::LAST_MODIFIED, &parts.headers)?;

        Ok(Some(GetObjectResponse {
            content_length,
            content_type,
            date,
            e_tag,
            last_modified,
            content,
        }))
    }

    pub(crate) async fn put_object_response(self) -> Result<PutObjectResponse, Error> {
        let (parts, body) = self.response.into_parts();
        let body = body.collect().await?.to_bytes();

        match parts.status {
            StatusCode::OK => (),
            StatusCode::PRECONDITION_FAILED => return Ok(PutObjectResponse::PreconditionFailed),
            StatusCode::CONFLICT => return Ok(PutObjectResponse::NeedsRetry),
            StatusCode::BAD_REQUEST => {
                Self::log_error_response_utf8(body);
                bail!("invalid request");
            }
            status_code => {
                Self::log_error_response_utf8(body);
                bail!("unexpected status code {status_code}")
            }
        };

        if !body.is_empty() {
            bail!("got unexpected non-empty response body");
        }

        let e_tag = Self::parse_header(header::ETAG, &parts.headers)?;

        Ok(PutObjectResponse::Success(e_tag))
    }

    pub(crate) async fn delete_object_response(self) -> Result<(), Error> {
        let (parts, _body) = self.response.into_parts();

        match parts.status {
            StatusCode::NO_CONTENT => (),
            status_code => bail!("unexpected status code {status_code}"),
        };

        Ok(())
    }

    pub(crate) async fn delete_objects_response(self) -> Result<DeleteObjectsResponse, Error> {
        let (parts, body) = self.response.into_parts();
        let body = body.collect().await?.to_bytes();

        match parts.status {
            StatusCode::OK => (),
            StatusCode::BAD_REQUEST => {
                Self::log_error_response_utf8(body);
                bail!("invalid request");
            }
            status_code => {
                Self::log_error_response_utf8(body);
                bail!("unexpected status code {status_code}")
            }
        };

        let body = String::from_utf8(body.to_vec())?;

        let delete_objects_response: DeleteObjectsResponse =
            serde_xml_rs::from_str(&body).context("failed to parse response body")?;

        Ok(delete_objects_response)
    }

    pub(crate) async fn copy_object_response(self) -> Result<CopyObjectResponse, Error> {
        let (parts, body) = self.response.into_parts();
        let body = body.collect().await?.to_bytes();

        match parts.status {
            StatusCode::OK => (),
            StatusCode::NOT_FOUND => bail!("object not found"),
            StatusCode::FORBIDDEN => bail!("the source object is not in the active tier"),
            status_code => {
                Self::log_error_response_utf8(body);
                bail!("unexpected status code {status_code}")
            }
        }

        let body = String::from_utf8(body.to_vec())?;

        let x_amz_version_id = match parts.headers.get("x-amz-version-id") {
            Some(version_id) => Some(
                version_id
                    .to_str()
                    .context("failed to parse version id header")?
                    .to_owned(),
            ),
            None => None,
        };

        let copy_object_result: CopyObjectResult =
            serde_xml_rs::from_str(&body).context("failed to parse response body")?;

        Ok(CopyObjectResponse {
            copy_object_result,
            x_amz_version_id,
        })
    }

    pub(crate) async fn list_buckets_response(self) -> Result<ListBucketsResponse, Error> {
        let (parts, body) = self.response.into_parts();
        let body = body.collect().await?.to_bytes();

        if !matches!(parts.status, StatusCode::OK) {
            Self::log_error_response_utf8(body);
            bail!("unexpected status code {}", parts.status);
        }

        let body = String::from_utf8(body.to_vec())?;

        let list_buckets_result: ListAllMyBucketsResult =
            serde_xml_rs::from_str(&body).context("failed to parse response body")?;

        let buckets = match list_buckets_result.buckets {
            Some(buckets) => buckets.bucket,
            None => Vec::new(),
        };
        Ok(ListBucketsResponse { buckets })
    }

    fn log_error_response_utf8(body: Bytes) {
        if let Ok(body) = String::from_utf8(body.to_vec()) {
            if !body.is_empty() {
                tracing::error!("{body}");
            }
        }
    }

    fn parse_header<T: FromStr>(name: HeaderName, headers: &HeaderMap) -> Result<T, Error>
    where
        <T as FromStr>::Err: Send + Sync + 'static,
        Result<T, <T as FromStr>::Err>: Context<T, <T as FromStr>::Err>,
    {
        let header_value = headers
            .get(&name)
            .ok_or_else(|| anyhow!("missing header '{name}'"))?;
        let header_str = header_value
            .to_str()
            .with_context(|| format!("non UTF-8 header '{name}'"))?;
        let value = header_str
            .parse()
            .with_context(|| format!("failed to parse header '{name}'"))?;
        Ok(value)
    }
}
