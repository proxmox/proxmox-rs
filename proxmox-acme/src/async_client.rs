//! Async HTTP Client implementation for the ACME protocol.

use anyhow::format_err;
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::Request;
use serde::{Deserialize, Serialize};

use proxmox_http::{client::Client, Body};

use crate::account::AccountCreator;
use crate::order::{Order, OrderData};
use crate::Request as AcmeRequest;
use crate::{Account, Authorization, Challenge, Directory, Error, ErrorResponse};

/// A non-blocking Acme client using tokio/hyper.
pub struct AcmeClient {
    directory_url: String,
    account: Option<Account>,
    directory: Option<Directory>,
    nonce: Option<String>,
    http_client: Client,
}

impl AcmeClient {
    /// Create a new ACME client for a given ACME directory URL.
    pub fn new(directory_url: String) -> Self {
        const USER_AGENT_STRING: &str = "proxmox-acme-client/1.0";
        const TCP_KEEPALIVE_TIME: u32 = 120;

        let options = proxmox_http::HttpOptions {
            proxy_config: None, // fixme???
            user_agent: Some(USER_AGENT_STRING.to_string()),
            tcp_keepalive: Some(TCP_KEEPALIVE_TIME),
        };

        let http_client = Client::with_options(options);

        Self {
            directory_url,
            account: None,
            directory: None,
            nonce: None,
            http_client,
        }
    }

    /// Get the current account, if there is one.
    pub fn account(&self) -> Option<&Account> {
        self.account.as_ref()
    }

    /// Set the account this client should use.
    pub fn set_account(&mut self, account: Account) {
        self.account = Some(account);
    }

    /// Convenience method to create a new account with a list of ACME compatible contact strings
    /// (eg. `mailto:someone@example.com`).
    ///
    /// Please remember to persist the returned `Account` structure somewhere to not lose access to
    /// the account!
    ///
    /// If an RSA key size is provided, an RSA key will be generated. Otherwise an EC key using the
    /// P-256 curve will be generated.
    pub async fn new_account(
        &mut self,
        tos_agreed: bool,
        contact: Vec<String>,
        rsa_bits: Option<u32>,
        eab_creds: Option<(String, String)>,
    ) -> Result<&Account, anyhow::Error> {
        let mut account = Account::creator()
            .set_contacts(contact)
            .agree_to_tos(tos_agreed);

        if let Some((eab_kid, eab_hmac_key)) = eab_creds {
            account = account.set_eab_credentials(eab_kid, eab_hmac_key)?;
        }

        let account = if let Some(bits) = rsa_bits {
            account.generate_rsa_key(bits)?
        } else {
            account.generate_ec_key()?
        };

        let _ = self.register_account(account).await?;

        // unwrap: Setting `self.account` is literally this function's job, we just can't keep
        // the borrow from from `self.register_account()` active due to clashes.
        Ok(self.account.as_ref().unwrap())
    }

    /// Shortcut to `account().ok_or_else(...).key_authorization()`.
    pub fn key_authorization(&self, token: &str) -> Result<String, anyhow::Error> {
        Ok(Self::need_account(&self.account)?.key_authorization(token)?)
    }

    /// Shortcut to `account().ok_or_else(...).dns_01_txt_value()`.
    /// the key authorization value.
    pub fn dns_01_txt_value(&self, token: &str) -> Result<String, anyhow::Error> {
        Ok(Self::need_account(&self.account)?.dns_01_txt_value(token)?)
    }

    async fn register_account(
        &mut self,
        account: AccountCreator,
    ) -> Result<&Account, anyhow::Error> {
        let mut retry = retry();
        let mut response = loop {
            retry.tick()?;

            let (directory, nonce) = Self::get_dir_nonce(
                &mut self.http_client,
                &self.directory_url,
                &mut self.directory,
                &mut self.nonce,
            )
            .await?;
            let request = account.request(directory, nonce)?;
            match self.run_request(request).await {
                Ok(response) => break response,
                Err(err) if err.is_bad_nonce() => continue,
                Err(err) => return Err(err.into()),
            }
        };

        let account = account.response(response.location_required()?, &response.body)?;

        self.account = Some(account);
        Ok(self.account.as_ref().unwrap())
    }

    /// Update account data.
    ///
    /// Low-level version: we allow arbitrary data to be passed to the remote here, it's up to the
    /// user to know what to do for now.
    pub async fn update_account<T: Serialize>(
        &mut self,
        data: &T,
    ) -> Result<&Account, anyhow::Error> {
        let account = Self::need_account(&self.account)?;

        let mut retry = retry();
        let response = loop {
            retry.tick()?;

            let (_directory, nonce) = Self::get_dir_nonce(
                &mut self.http_client,
                &self.directory_url,
                &mut self.directory,
                &mut self.nonce,
            )
            .await?;

            let request = account.post_request(&account.location, nonce, data)?;
            match Self::execute(&mut self.http_client, request, &mut self.nonce).await {
                Ok(response) => break response,
                Err(err) if err.is_bad_nonce() => continue,
                Err(err) => return Err(err.into()),
            }
        };

        // unwrap: we've been keeping an immutable reference to it from the top of the method
        let _ = account;
        self.account.as_mut().unwrap().data = response.json()?;
        // fixme: self.save()?;
        Ok(self.account.as_ref().unwrap())
    }

    /// Method to create a new order for a set of domains.
    ///
    /// Please remember to persist the order somewhere (ideally along with the account data) in
    /// order to finish & query it later on.
    pub async fn new_order<I>(&mut self, domains: I) -> Result<Order, anyhow::Error>
    where
        I: IntoIterator<Item = String>,
    {
        let account = Self::need_account(&self.account)?;

        let order = domains
            .into_iter()
            .fold(OrderData::new(), |order, domain| order.domain(domain));

        let mut retry = retry();
        loop {
            retry.tick()?;

            let (directory, nonce) = Self::get_dir_nonce(
                &mut self.http_client,
                &self.directory_url,
                &mut self.directory,
                &mut self.nonce,
            )
            .await?;

            let mut new_order = account.new_order(&order, directory, nonce)?;
            let mut response = match Self::execute(
                &mut self.http_client,
                new_order.request.take().unwrap(),
                &mut self.nonce,
            )
            .await
            {
                Ok(response) => response,
                Err(err) if err.is_bad_nonce() => continue,
                Err(err) => return Err(err.into()),
            };

            return Ok(
                new_order.response(response.location_required()?, response.bytes().as_ref())?
            );
        }
    }

    /// Low level "POST-as-GET" request.
    async fn post_as_get(&mut self, url: &str) -> Result<AcmeResponse, anyhow::Error> {
        let account = Self::need_account(&self.account)?;

        let mut retry = retry();
        loop {
            retry.tick()?;

            let (_directory, nonce) = Self::get_dir_nonce(
                &mut self.http_client,
                &self.directory_url,
                &mut self.directory,
                &mut self.nonce,
            )
            .await?;

            let request = account.get_request(url, nonce)?;
            match Self::execute(&mut self.http_client, request, &mut self.nonce).await {
                Ok(response) => return Ok(response),
                Err(err) if err.is_bad_nonce() => continue,
                Err(err) => return Err(err.into()),
            }
        }
    }

    /// Low level POST request.
    async fn post<T: Serialize>(
        &mut self,
        url: &str,
        data: &T,
    ) -> Result<AcmeResponse, anyhow::Error> {
        let account = Self::need_account(&self.account)?;

        let mut retry = retry();
        loop {
            retry.tick()?;

            let (_directory, nonce) = Self::get_dir_nonce(
                &mut self.http_client,
                &self.directory_url,
                &mut self.directory,
                &mut self.nonce,
            )
            .await?;

            let request = account.post_request(url, nonce, data)?;
            match Self::execute(&mut self.http_client, request, &mut self.nonce).await {
                Ok(response) => return Ok(response),
                Err(err) if err.is_bad_nonce() => continue,
                Err(err) => return Err(err.into()),
            }
        }
    }

    /// Request challenge validation. Afterwards, the challenge should be polled.
    pub async fn request_challenge_validation(
        &mut self,
        url: &str,
    ) -> Result<Challenge, anyhow::Error> {
        Ok(self
            .post(url, &serde_json::Value::Object(Default::default()))
            .await?
            .json()?)
    }

    /// Assuming the provided URL is an 'Authorization' URL, get and deserialize it.
    pub async fn get_authorization(&mut self, url: &str) -> Result<Authorization, anyhow::Error> {
        Ok(self.post_as_get(url).await?.json()?)
    }

    /// Assuming the provided URL is an 'Order' URL, get and deserialize it.
    pub async fn get_order(&mut self, url: &str) -> Result<OrderData, anyhow::Error> {
        Ok(self.post_as_get(url).await?.json()?)
    }

    /// Finalize an Order via its `finalize` URL property and the DER encoded CSR.
    pub async fn finalize(&mut self, url: &str, csr: &[u8]) -> Result<(), anyhow::Error> {
        let csr = proxmox_base64::url::encode_no_pad(csr);
        let data = serde_json::json!({ "csr": csr });
        self.post(url, &data).await?;
        Ok(())
    }

    /// Download a certificate via its 'certificate' URL property.
    ///
    /// The certificate will be a PEM certificate chain.
    pub async fn get_certificate(&mut self, url: &str) -> Result<Bytes, anyhow::Error> {
        Ok(self.post_as_get(url).await?.body)
    }

    /// Revoke an existing certificate (PEM or DER formatted).
    pub async fn revoke_certificate(
        &mut self,
        certificate: &[u8],
        reason: Option<u32>,
    ) -> Result<(), anyhow::Error> {
        // TODO: This can also work without an account.
        let account = Self::need_account(&self.account)?;

        let revocation = account.revoke_certificate(certificate, reason)?;

        let mut retry = retry();
        loop {
            retry.tick()?;

            let (directory, nonce) = Self::get_dir_nonce(
                &mut self.http_client,
                &self.directory_url,
                &mut self.directory,
                &mut self.nonce,
            )
            .await?;

            let request = revocation.request(directory, nonce)?;
            match Self::execute(&mut self.http_client, request, &mut self.nonce).await {
                Ok(_response) => return Ok(()),
                Err(err) if err.is_bad_nonce() => continue,
                Err(err) => return Err(err.into()),
            }
        }
    }

    fn need_account(account: &Option<Account>) -> Result<&Account, anyhow::Error> {
        account
            .as_ref()
            .ok_or_else(|| format_err!("cannot use client without an account"))
    }

    /// Get the directory URL without querying the `Directory` structure.
    ///
    /// The difference to [`directory`](AcmeClient::directory()) is that this does not
    /// attempt to fetch the directory data from the ACME server.
    pub fn directory_url(&self) -> &str {
        &self.directory_url
    }
}

struct AcmeResponse {
    body: Bytes,
    location: Option<String>,
    got_nonce: bool,
}

impl AcmeResponse {
    /// Convenience helper to assert that a location header was part of the response.
    fn location_required(&mut self) -> Result<String, anyhow::Error> {
        self.location
            .take()
            .ok_or_else(|| format_err!("missing Location header"))
    }

    /// Convenience shortcut to perform json deserialization of the returned body.
    fn json<T: for<'a> Deserialize<'a>>(&self) -> Result<T, Error> {
        Ok(serde_json::from_slice(&self.body)?)
    }

    /// Convenience shortcut to get the body as bytes.
    fn bytes(&self) -> &[u8] {
        &self.body
    }
}

impl AcmeClient {
    /// Non-self-borrowing run_request version for borrow workarounds.
    async fn execute(
        http_client: &mut Client,
        request: AcmeRequest,
        nonce: &mut Option<String>,
    ) -> Result<AcmeResponse, Error> {
        let req_builder = Request::builder().method(request.method).uri(&request.url);

        let http_request = if !request.content_type.is_empty() {
            req_builder
                .header("Content-Type", request.content_type)
                .header("Content-Length", request.body.len())
                .body(request.body.into())
        } else {
            req_builder.body(Body::empty())
        }
        .map_err(|err| Error::Custom(format!("failed to create http request: {err}")))?;

        let response = http_client
            .request(http_request)
            .await
            .map_err(|err| Error::Custom(err.to_string()))?;
        let (parts, body) = response.into_parts();

        let status = parts.status.as_u16();
        let body = body
            .collect()
            .await
            .map_err(|err| Error::Custom(format!("failed to retrieve response body: {err}")))?
            .to_bytes();

        let got_nonce = if let Some(new_nonce) = parts.headers.get(crate::REPLAY_NONCE) {
            let new_nonce = new_nonce.to_str().map_err(|err| {
                Error::Client(format!(
                    "received invalid replay-nonce header from ACME server: {err}"
                ))
            })?;
            *nonce = Some(new_nonce.to_owned());
            true
        } else {
            false
        };

        if parts.status.is_success() {
            if status != request.expected {
                return Err(Error::InvalidApi(format!(
                    "ACME server responded with unexpected status code: {:?}",
                    parts.status
                )));
            }

            let location = parts
                .headers
                .get("Location")
                .map(|header| {
                    header.to_str().map(str::to_owned).map_err(|err| {
                        Error::Client(format!(
                            "received invalid location header from ACME server: {err}"
                        ))
                    })
                })
                .transpose()?;

            return Ok(AcmeResponse {
                body,
                location,
                got_nonce,
            });
        }

        let error: ErrorResponse = serde_json::from_slice(&body).map_err(|err| {
            Error::Client(format!(
                "error status with improper error ACME response: {err}"
            ))
        })?;

        if error.ty == crate::error::BAD_NONCE {
            if !got_nonce {
                return Err(Error::InvalidApi(
                    "badNonce without a new Replay-Nonce header".to_string(),
                ));
            }
            return Err(Error::BadNonce);
        }

        Err(Error::Api(error))
    }

    /// Low-level API to run an n API request. This automatically updates the current nonce!
    async fn run_request(&mut self, request: AcmeRequest) -> Result<AcmeResponse, Error> {
        Self::execute(&mut self.http_client, request, &mut self.nonce).await
    }

    /// Get the Directory information.
    pub async fn directory(&mut self) -> Result<&Directory, Error> {
        Ok(Self::get_directory(
            &mut self.http_client,
            &self.directory_url,
            &mut self.directory,
            &mut self.nonce,
        )
        .await?
        .0)
    }

    async fn get_directory<'a, 'b>(
        http_client: &mut Client,
        directory_url: &str,
        directory: &'a mut Option<Directory>,
        nonce: &'b mut Option<String>,
    ) -> Result<(&'a Directory, Option<&'b str>), Error> {
        if let Some(d) = directory {
            return Ok((d, nonce.as_deref()));
        }

        let response = Self::execute(
            http_client,
            AcmeRequest {
                url: directory_url.to_string(),
                method: "GET",
                content_type: "",
                body: String::new(),
                expected: 200,
            },
            nonce,
        )
        .await?;

        *directory = Some(Directory::from_parts(
            directory_url.to_string(),
            response.json()?,
        ));

        Ok((directory.as_ref().unwrap(), nonce.as_deref()))
    }

    /// Like `get_directory`, but if the directory provides no nonce, also performs a `HEAD`
    /// request on the new nonce URL.
    async fn get_dir_nonce<'a, 'b>(
        http_client: &mut Client,
        directory_url: &str,
        directory: &'a mut Option<Directory>,
        nonce: &'b mut Option<String>,
    ) -> Result<(&'a Directory, &'b str), Error> {
        // this let construct is a lifetime workaround:
        let _ = Self::get_directory(http_client, directory_url, directory, nonce).await?;
        let dir = directory.as_ref().unwrap(); // the above fails if it couldn't fill this option
        if nonce.is_none() {
            // this is also a lifetime issue...
            let _ = Self::get_nonce(http_client, nonce, dir.new_nonce_url()).await?;
        };
        Ok((dir, nonce.as_deref().unwrap()))
    }

    /// Convenience method to get the ToS URL from the contained `Directory`.
    ///
    /// This requires mutable self as the directory information may be lazily loaded, which can
    /// fail.
    pub async fn terms_of_service_url(&mut self) -> Result<Option<&str>, Error> {
        Ok(self.directory().await?.terms_of_service_url())
    }

    async fn get_nonce<'a>(
        http_client: &mut Client,
        nonce: &'a mut Option<String>,
        new_nonce_url: &str,
    ) -> Result<&'a str, Error> {
        let response = Self::execute(
            http_client,
            AcmeRequest {
                url: new_nonce_url.to_owned(),
                method: "HEAD",
                content_type: "",
                body: String::new(),
                expected: 200,
            },
            nonce,
        )
        .await?;

        if !response.got_nonce {
            return Err(Error::InvalidApi(
                "no new nonce received from new nonce URL".to_string(),
            ));
        }

        nonce
            .as_deref()
            .ok_or_else(|| Error::Client("failed to update nonce".to_string()))
    }
}

/// bad nonce retry count helper
struct Retry(usize);

const fn retry() -> Retry {
    Retry(0)
}

impl Retry {
    fn tick(&mut self) -> Result<(), Error> {
        if self.0 >= 3 {
            Err(Error::Client("kept getting a badNonce error!".to_string()))
        } else {
            self.0 += 1;
            Ok(())
        }
    }
}
