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

/// Response reader to check S3 api response status codes and parse response body, if any.
pub(crate) struct ResponseReader {
    response: Response<Incoming>,
}

#[derive(Debug)]
/// Response contents of list objects v2 api calls.
pub struct ListObjectsV2Response {
    /// Parsed http date header from response.
    pub date: HttpDate,
    /// Bucket name.
    pub name: String,
    /// Requested key prefix.
    pub prefix: String,
    /// Number of keys returned in this response.
    pub key_count: u64,
    /// Number of max keys the response can contain.
    pub max_keys: u64,
    /// Flag indication if response was truncated because of key limits.
    pub is_truncated: bool,
    /// Token used for this request to get further keys in truncated responses.
    pub continuation_token: Option<String>,
    /// Allows to fetch the next set of keys for truncated responses.
    pub next_continuation_token: Option<String>,
    /// List of response objects, including their object key.
    pub contents: Vec<ListObjectsV2Contents>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
/// Subset of items used to deserialize a list objects v2 respsonse.
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListObjectsV2.html#API_ListObjectsV2_ResponseSyntax
struct ListObjectsV2ResponseBody {
    /// Bucket name.
    pub name: String,
    /// Requested key prefix.
    pub prefix: String,
    /// Number of keys returned in this response.
    pub key_count: u64,
    /// Number of max keys the response can contain.
    pub max_keys: u64,
    /// Flag indication if response was truncated because of key limits.
    pub is_truncated: bool,
    /// Token used for this request to get further keys in truncated responses.
    pub continuation_token: Option<String>,
    /// Allows to fetch the next set of keys for truncated responses.
    pub next_continuation_token: Option<String>,
    /// List of response objects, including their object key.
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
/// Subset of contents used to deserialize the listed object contents of a list objects v2 respsonse.
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListObjectsV2.html#API_ListObjectsV2_ResponseSyntax
pub struct ListObjectsV2Contents {
    /// Object key.
    pub key: S3ObjectKey,
    /// Object last modified timestamp.
    pub last_modified: LastModifiedTimestamp,
    /// Entity tag for object.
    pub e_tag: String,
    /// Content size of object.
    pub size: u64,
    /// Storage class the object is stored on.
    pub storage_class: String,
}

#[derive(Debug)]
/// Subset of contents for the head object response (headers only, there is no body).
/// See https://docs.aws.amazon.com/AmazonS3/latest/API/API_HeadObject.html#API_HeadObject_ResponseSyntax
pub struct HeadObjectResponse {
    /// Content length header.
    pub content_length: u64,
    /// Content type header.
    pub content_type: String,
    /// Http date header.
    pub date: HttpDate,
    /// Entity tag header.
    pub e_tag: String,
    /// Last modified http header.
    pub last_modified: HttpDate,
}

/// Response contents of the get object api call.
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_GetObject.html#API_GetObject_ResponseSyntax
pub struct GetObjectResponse {
    /// Content length header.
    pub content_length: u64,
    /// Content type header.
    pub content_type: String,
    /// Http date header.
    pub date: HttpDate,
    /// Entity tag header.
    pub e_tag: String,
    /// Last modified http header.
    pub last_modified: HttpDate,
    /// Object content in http response body.
    pub content: Incoming,
}

#[derive(Debug)]
/// Variants to distinguish object upload response states.
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_PutObject.html#API_PutObject_ResponseSyntax
pub enum PutObjectResponse {
    /// Object upload failed because of conflicting operation, upload should be retried.
    NeedsRetry,
    /// Object was not uploaded because the provided pre-condition
    /// (e.g. If-None-Match header) failed.
    PreconditionFailed,
    /// Object was uploaded with success with the contained entity tag.
    Success(String),
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "PascalCase")]
/// Response contents of the delete objects api call.
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_DeleteObjects.html#API_DeleteObjects_ResponseElements
pub struct DeleteObjectsResponse {
    /// List of deleted objects, if any.
    pub deleted: Option<Vec<DeletedObject>>,
    /// List of errors, if deletion failed for some objects.
    pub error: Option<Vec<DeleteObjectError>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
/// Subset of contents used to deserialize the deleted objects of a delete objects v2 respsonse.
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_DeletedObject.html
pub struct DeletedObject {
    /// Indicates whether the version of the object was a delete marker before deletion.
    pub delete_marker: Option<bool>,
    /// Version ID of the delete marker created as result of the delete operation.
    pub delete_marker_version_id: Option<String>,
    /// Key of the deleted object.
    pub key: Option<S3ObjectKey>,
    /// Version ID of the deleted object.
    pub version_id: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
/// Subset of contents used to deserialize the deleted object errors of a delete objects v2 respsonse
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_Error.html
pub struct DeleteObjectError {
    /// Error code identifying the error condition.
    pub code: Option<String>,
    /// Object key for which the error occurred.
    pub key: Option<S3ObjectKey>,
    /// Generic error description.
    pub message: Option<String>,
    /// Version ID of error.
    pub version_id: Option<String>,
}

#[derive(Debug)]
/// Response contents of the copy object api calls.
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_CopyObject.html#API_CopyObject_ResponseSyntax
pub struct CopyObjectResponse {
    /// Result contents of the copy object operation.
    pub copy_object_result: CopyObjectResult,
    /// Version ID of the newly created object copy.
    pub x_amz_version_id: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
/// Subset of contents used to deserialize the copy object result of a copy object respsonse.
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_CopyObject.html#API_CopyObject_ResponseSyntax
pub struct CopyObjectResult {
    /// Entity tag.
    pub e_tag: String,
    /// Last modified timestamp.
    pub last_modified: LastModifiedTimestamp,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
/// Response contents of the list buckets api calls.
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListBuckets.html#API_ListBuckets_ResponseElements
pub struct ListBucketsResponse {
    /// List of buckets accessible given caller's access key.
    pub buckets: Vec<Bucket>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
/// Subset of contents used to deserialize the response of a list buckets api call.
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListBuckets.html#API_ListBuckets_ResponseElements
pub struct ListAllMyBucketsResult {
    /// List Bucket contents.
    pub buckets: Option<Buckets>,
}

/// Subset of contents used to deserialize the list of buckets for response of a list buckets api
/// call.
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListBuckets.html#API_ListBuckets_ResponseElements
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Buckets {
    /// List of individual bucket contents.
    bucket: Vec<Bucket>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
/// Subset of contents used to deserialize individual buckets for response of a list buckets api
/// call.
/// https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListBuckets.html#API_ListBuckets_ResponseElements
pub struct Bucket {
    /// Bucket name.
    pub name: String,
    // Bucket ARN.
    pub bucket_arn: Option<String>,
    /// Bucket region.
    pub bucket_region: Option<String>,
    /// Bucket creation timestamp.
    pub creation_date: LastModifiedTimestamp,
}

impl ResponseReader {
    /// Create a new response reader to parse given response.
    pub(crate) fn new(response: Response<Incoming>) -> Self {
        Self { response }
    }

    /// Read and parse the list object v2 response.
    ///
    /// Returns with error if the bucket cannot be found, an unexpected status code is encountered
    /// or the response body cannot be parsed.
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

    /// Read and parse the head object response.
    ///
    /// Returns with error if an unexpected status code is encountered or the response headers or
    /// body cannot be parsed.
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

    /// Read and parse the get object response.
    ///
    /// Returns with error if the object is not accessible, an unexpected status code is encountered
    /// or the response headers or body cannot be parsed.
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

    /// Read and parse the put object response.
    ///
    /// Returns with error on bad request, an unexpected status code is encountered or the response
    /// headers or body cannot be parsed.
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

    /// Read and parse the delete object response.
    ///
    /// Returns with error if an unexpected status code is encountered.
    pub(crate) async fn delete_object_response(self) -> Result<(), Error> {
        let (parts, _body) = self.response.into_parts();

        match parts.status {
            StatusCode::NO_CONTENT => (),
            status_code => bail!("unexpected status code {status_code}"),
        };

        Ok(())
    }

    /// Read and parse the delete objects response.
    ///
    /// Returns with error on bad request, an unexpected status code is encountered or the response
    /// body cannot be parsed.
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

    /// Read and parse the copy object response.
    ///
    /// Returns with error if the source object cannot be found or is in-accessible, an unexpected
    /// status code is encountered or the response headers or body cannot be parsed.
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

    /// Read and parse the list buckets response.
    ///
    /// Returns with error if an unexpected status code is encountered or the response body cannot
    /// be parsed.
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

        let buckets = list_buckets_result
            .buckets
            .map(|b| b.bucket)
            .unwrap_or_default();
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
