use std::convert::TryFrom;

use curl::easy;
use serde::{Deserialize, Serialize};

use crate::b64u;
use crate::error;
use crate::order::OrderData;
use crate::request::ErrorResponse;
use crate::{Account, Authorization, Challenge, Directory, Error, Order, Request};

macro_rules! format_err {
    ($($fmt:tt)*) => { Error::Client(format!($($fmt)*)) };
}

macro_rules! bail {
    ($($fmt:tt)*) => {{ return Err(format_err!($($fmt)*)); }}
}

/// Low level HTTP response structure.
pub struct HttpResponse {
    pub body: Vec<u8>,
    pub status: u16,
    pub headers: Headers,
}

impl HttpResponse {
    /// Check the HTTP status code for a success code (200..299).
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// Convenience shortcut to perform json deserialization of the returned body.
    pub fn json<T: for<'a> Deserialize<'a>>(&self) -> Result<T, Error> {
        Ok(serde_json::from_slice(&self.body)?)
    }

    /// Access the raw body as bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.body
    }

    /// Get the returned location header. Borrowing shortcut to `self.headers.location`.
    pub fn location(&self) -> Option<&str> {
        self.headers.location.as_deref()
    }

    /// Convenience helper to assert that a location header was part of the response.
    pub fn location_required(&mut self) -> Result<String, Error> {
        self.headers
            .location
            .take()
            .ok_or_else(|| format_err!("missing Location header"))
    }
}

/// Contains headers from the HTTP response which are relevant parts of the Acme API.
///
/// Note that access to the `nonce` header is internal to this crate only, since a nonce will
/// always be moved out of the response into the `Client` whenever a new nonce is received.
#[derive(Default)]
pub struct Headers {
    pub location: Option<String>,
    nonce: Option<String>,
}

impl Headers {
    fn read_header(&mut self, header: &[u8]) {
        let (name, value) = match parse_header(header) {
            Some(h) => h,
            None => return,
        };

        if name.eq_ignore_ascii_case(crate::REPLAY_NONCE) {
            self.nonce = Some(value.to_owned());
        } else if name.eq_ignore_ascii_case(crate::LOCATION) {
            self.location = Some(value.to_owned());
        }
    }
}

struct Inner {
    easy: easy::Easy,
    nonce: Option<String>,
}

impl Inner {
    pub fn new() -> Self {
        Self {
            easy: easy::Easy::new(),
            nonce: None,
        }
    }

    pub fn execute(
        &mut self,
        method: &[u8],
        url: &str,
        request_body: Option<&[u8]>,
    ) -> Result<HttpResponse, Error> {
        let mut body = Vec::new();
        let mut headers = Headers::default();
        let mut upload;

        match method {
            b"POST" => self.easy.post(true)?,
            b"GET" => self.easy.get(true)?,
            b"HEAD" => self.easy.nobody(true)?,
            other => bail!("invalid http method: {:?}", other),
        }

        self.easy.url(url)?;

        {
            let mut transfer = self.easy.transfer();

            transfer.write_function(|data| {
                body.extend(data);
                Ok(data.len())
            })?;

            transfer.header_function(|data| {
                headers.read_header(data);
                true
            })?;

            if let Some(body) = request_body {
                upload = body;
                transfer.read_function(|dest| {
                    let len = upload.len().min(dest.len());
                    dest[..len].copy_from_slice(&upload[..len]);
                    upload = &upload[len..];
                    Ok(len)
                })?;
            }

            transfer.perform()?;
        }

        let status = self.easy.response_code()?;
        let status =
            u16::try_from(status).map_err(|_| format_err!("invalid status code: {}", status))?;
        Ok(HttpResponse {
            body,
            status,
            headers,
        })
    }

    /// Low-level API to run an n API request. This automatically updates the current nonce!
    fn run_request(&mut self, request: Request) -> Result<HttpResponse, Error> {
        self.easy.reset();

        let body = if !request.content_type.is_empty() {
            let mut headers = easy::List::new();
            headers.append(&format!("Content-Type: {}", request.content_type))?;
            self.easy
                .http_headers(headers)
                .map_err(|err| format_err!("curl error: {}", err))?;
            Some(request.body.as_bytes())
        } else {
            None
        };

        let mut response = self
            .execute(request.method.as_bytes(), &request.url, body)
            .map_err({
                // borrow fixup:
                let method = &request.method;
                let url = &request.url;
                move |err| format_err!("failed to execute {} request to {}: {}", method, url, err)
            })?;

        let got_nonce = self.update_nonce(&mut response)?;

        if response.is_success() {
            if response.status != request.expected {
                return Err(Error::InvalidApi(format!(
                    "API server responded with unexpected status code: {:?}",
                    response.status
                )));
            }
            return Ok(response);
        }

        let error: ErrorResponse = response.json().map_err(|err| {
            format_err!("error status with improper error ACME response: {}", err)
        })?;

        if error.ty == error::BAD_NONCE {
            if !got_nonce {
                return Err(Error::InvalidApi(
                    "badNonce without a new Replay-Nonce header".to_string(),
                ));
            }
            return Err(Error::BadNonce);
        }

        Err(Error::Api(error))
    }

    /// If the response contained a nonce, update our nonce and return `true`, otherwise return
    /// `false`.
    fn update_nonce(&mut self, response: &mut HttpResponse) -> Result<bool, Error> {
        match response.headers.nonce.take() {
            Some(nonce) => {
                self.nonce = Some(nonce);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Update the nonce, if there isn't one it is an error.
    fn must_update_nonce(&mut self, response: &mut HttpResponse) -> Result<(), Error> {
        if !self.update_nonce(response)? {
            bail!("newNonce URL did not return a nonce");
        }
        Ok(())
    }

    /// Update the Nonce.
    fn new_nonce(&mut self, new_nonce_url: &str) -> Result<(), Error> {
        let mut response = self.execute(b"HEAD", new_nonce_url, None).map_err(|err| {
            Error::InvalidApi(format!("failed to get HEAD of newNonce URL: {}", err))
        })?;

        if !response.is_success() {
            bail!("HEAD on newNonce URL returned error");
        }

        self.must_update_nonce(&mut response)?;

        Ok(())
    }

    /// Make sure a nonce is available without forcing renewal.
    fn nonce(&mut self, new_nonce_url: &str) -> Result<&str, Error> {
        if self.nonce.is_none() {
            self.new_nonce(new_nonce_url)?;
        }
        self.nonce
            .as_deref()
            .ok_or_else(|| format_err!("failed to get nonce"))
    }
}

/// A blocking Acme client using curl's `Easy` interface.
pub struct Client {
    inner: Inner,
    directory: Option<Directory>,
    account: Option<Account>,
    directory_url: String,
}

impl Client {
    /// Create a new Client. This has no account associated with it yet, so the next step is to
    /// either attach an existing `Account` or create a new one.
    pub fn new(directory_url: String) -> Self {
        Self {
            inner: Inner::new(),
            directory: None,
            account: None,
            directory_url,
        }
    }

    /// Set the account this client should use.
    pub fn set_account(&mut self, account: Account) {
        self.account = Some(account);
    }

    /// Get the Directory information.
    pub fn directory(&mut self) -> Result<&Directory, Error> {
        Self::get_directory(&mut self.inner, &mut self.directory, &self.directory_url)
    }

    /// Get the Directory information.
    fn get_directory<'a>(
        inner: &'_ mut Inner,
        directory: &'a mut Option<Directory>,
        directory_url: &str,
    ) -> Result<&'a Directory, Error> {
        if let Some(d) = directory {
            return Ok(d);
        }

        let response = inner
            .execute(b"GET", directory_url, None)
            .map_err(|err| Error::InvalidApi(format!("failed to get directory info: {}", err)))?;

        if !response.is_success() {
            bail!(
                "GET on the directory URL returned error status ({})",
                response.status
            );
        }

        *directory = Some(Directory::from_parts(
            directory_url.to_string(),
            response.json()?,
        ));
        Ok(directory.as_ref().unwrap())
    }

    /// Get the current account, if there is one.
    pub fn account(&self) -> Option<&Account> {
        self.account.as_ref()
    }

    /// Convenience method to get the ToS URL from the contained `Directory`.
    ///
    /// This requires mutable self as the directory information may be lazily loaded, which can
    /// fail.
    pub fn terms_of_service_url(&mut self) -> Result<Option<&str>, Error> {
        Ok(self.directory()?.terms_of_service_url())
    }

    /// Get a fresh nonce (this should normally not be required as nonces are updated
    /// automatically, even when a `badNonce` error occurs, which according to the ACME API
    /// specification should include a new valid nonce in its headers anyway).
    pub fn new_nonce(&mut self) -> Result<(), Error> {
        let was_none = self.inner.nonce.is_none();
        let directory =
            Self::get_directory(&mut self.inner, &mut self.directory, &self.directory_url)?;
        if was_none && self.inner.nonce.is_some() {
            // this was the first call and we already got a nonce from querying the directory
            return Ok(());
        }

        // otherwise actually call up to get a new nonce
        self.inner.new_nonce(directory.new_nonce_url())
    }

    /// borrow helper
    fn nonce<'a>(inner: &'a mut Inner, directory: &'_ Directory) -> Result<&'a str, Error> {
        inner.nonce(directory.new_nonce_url())
    }

    /// Convenience method to create a new account with a list of ACME compatible contact strings
    /// (eg. `mailto:someone@example.com`).
    ///
    /// Please remember to persist the returned `Account` structure somewhere to not lose access to
    /// the account!
    ///
    /// If an RSA key size is provided, an RSA key will be generated. Otherwise an EC key using the
    /// P-256 curve will be generated.
    pub fn new_account(
        &mut self,
        contact: Vec<String>,
        tos_agreed: bool,
        rsa_bits: Option<u32>,
    ) -> Result<&Account, Error> {
        let account = Account::creator()
            .set_contacts(contact)
            .agree_to_tos(tos_agreed);
        let account = if let Some(bits) = rsa_bits {
            account.generate_rsa_key(bits)?
        } else {
            account.generate_ec_key()?
        };

        self.register_account(account)
    }

    pub fn register_account(
        &mut self,
        account: crate::account::AccountCreator,
    ) -> Result<&Account, Error> {
        let mut retry = retry();
        let mut response = loop {
            retry.tick()?;

            let directory =
                Self::get_directory(&mut self.inner, &mut self.directory, &self.directory_url)?;
            let nonce = Self::nonce(&mut self.inner, directory)?;
            let request = account.request(directory, nonce)?;
            match self.run_request(request) {
                Ok(response) => break response,
                Err(err) if err.is_bad_nonce() => continue,
                Err(err) => return Err(err.into()),
            }
        };

        let account = account.response(response.location_required()?, response.bytes().as_ref())?;

        self.account = Some(account);
        Ok(self.account.as_ref().unwrap())
    }

    fn need_account(account: &Option<Account>) -> Result<&Account, Error> {
        account
            .as_ref()
            .ok_or_else(|| format_err!("cannot use client without an account"))
    }

    /// Update account data.
    ///
    /// Low-level version: we allow arbitrary data to be passed to the remote here, it's up to the
    /// user to know what to do for now.
    pub fn update_account<T: Serialize>(&mut self, data: &T) -> Result<&Account, Error> {
        let account = Self::need_account(&self.account)?;

        let mut retry = retry();
        let response = loop {
            retry.tick()?;
            let directory =
                Self::get_directory(&mut self.inner, &mut self.directory, &self.directory_url)?;
            let nonce = Self::nonce(&mut self.inner, directory)?;
            let request = account.post_request(&account.location, &nonce, data)?;
            let response = match self.inner.run_request(request) {
                Ok(response) => response,
                Err(err) if err.is_bad_nonce() => continue,
                Err(err) => return Err(err.into()),
            };

            break response;
        };

        // unwrap: we asserted we have an account at the top of the method!
        let account = self.account.as_mut().unwrap();
        account.data = response.json()?;
        Ok(account)
    }

    /// Method to create a new order for a set of domains.
    ///
    /// Please remember to persist the order somewhere (ideally along with the account data) in
    /// order to finish & query it later on.
    pub fn new_order(&mut self, domains: Vec<String>) -> Result<Order, Error> {
        let account = Self::need_account(&self.account)?;

        let order = domains
            .into_iter()
            .fold(OrderData::new(), |order, domain| order.domain(domain));

        let mut retry = retry();
        loop {
            retry.tick()?;

            let directory =
                Self::get_directory(&mut self.inner, &mut self.directory, &self.directory_url)?;
            let nonce = Self::nonce(&mut self.inner, directory)?;
            let mut new_order = account.new_order(&order, directory, nonce)?;
            let mut response = match self.inner.run_request(new_order.request.take().unwrap()) {
                Ok(response) => response,
                Err(err) if err.is_bad_nonce() => continue,
                Err(err) => return Err(err.into()),
            };

            return new_order.response(response.location_required()?, response.bytes().as_ref());
        }
    }

    /// Assuming the provided URL is an 'Authorization' URL, get and deserialize it.
    pub fn get_authorization(&mut self, url: &str) -> Result<Authorization, Error> {
        Ok(self.post_as_get(url)?.json()?)
    }

    /// Assuming the provided URL is an 'Order' URL, get and deserialize it.
    pub fn get_order(&mut self, url: &str) -> Result<OrderData, Error> {
        Ok(self.post_as_get(url)?.json()?)
    }

    /// Low level "POST-as-GET" request.
    pub fn post_as_get(&mut self, url: &str) -> Result<HttpResponse, Error> {
        let account = Self::need_account(&self.account)?;

        let mut retry = retry();
        loop {
            retry.tick()?;

            let directory =
                Self::get_directory(&mut self.inner, &mut self.directory, &self.directory_url)?;
            let nonce = Self::nonce(&mut self.inner, directory)?;
            let request = account.get_request(url, nonce)?;
            match self.inner.run_request(request) {
                Ok(response) => return Ok(response),
                Err(err) if err.is_bad_nonce() => continue,
                Err(err) => return Err(err.into()),
            }
        }
    }

    /// Low level POST request.
    pub fn post<T: Serialize>(&mut self, url: &str, data: &T) -> Result<HttpResponse, Error> {
        let account = Self::need_account(&self.account)?;

        let mut retry = retry();
        loop {
            retry.tick()?;

            let directory =
                Self::get_directory(&mut self.inner, &mut self.directory, &self.directory_url)?;
            let nonce = Self::nonce(&mut self.inner, directory)?;
            let request = account.post_request(url, nonce, data)?;
            match self.inner.run_request(request) {
                Ok(response) => return Ok(response),
                Err(err) if err.is_bad_nonce() => continue,
                Err(err) => return Err(err.into()),
            }
        }
    }

    /// Request challenge validation. Afterwards, the challenge should be polled.
    pub fn request_challenge_validation(&mut self, url: &str) -> Result<Challenge, Error> {
        Ok(self.post(url, &serde_json::json!({}))?.json()?)
    }

    /// Shortcut to `account().ok_or_else(...).key_authorization()`.
    pub fn key_authorization(&self, token: &str) -> Result<String, Error> {
        Self::need_account(&self.account)?.key_authorization(token)
    }

    /// Shortcut to `account().ok_or_else(...).dns_01_txt_value()`.
    /// the key authorization value.
    pub fn dns_01_txt_value(&self, token: &str) -> Result<String, Error> {
        Self::need_account(&self.account)?.dns_01_txt_value(token)
    }

    /// Low-level API to run an n API request. This automatically updates the current nonce!
    pub fn run_request(&mut self, request: Request) -> Result<HttpResponse, Error> {
        self.inner.run_request(request)
    }

    /// Finalize an Order via its `finalize` URL property and the DER encoded CSR.
    pub fn finalize(&mut self, url: &str, csr: &[u8]) -> Result<(), Error> {
        let csr = b64u::encode(csr);
        let data = serde_json::json!({ "csr": csr });
        self.post(url, &data)?;
        Ok(())
    }

    /// Download a certificate via its 'certificate' URL property.
    ///
    /// The certificate will be a PEM certificate chain.
    pub fn get_certificate(&mut self, url: &str) -> Result<Vec<u8>, Error> {
        Ok(self.post_as_get(url)?.body)
    }

    /// Revoke an existing certificate (PEM or DER formatted).
    pub fn revoke_certificate(
        &mut self,
        certificate: &[u8],
        reason: Option<u32>,
    ) -> Result<(), Error> {
        // TODO: This can also work without an account.
        let account = Self::need_account(&self.account)?;

        let cert = if certificate.starts_with(b"-----BEGIN CERTIFICATE-----") {
            b64u::encode(&openssl::x509::X509::from_pem(certificate)?.to_der()?)
        } else {
            b64u::encode(certificate)
        };

        let data = match reason {
            Some(reason) => serde_json::json!({ "certificate": cert, "reason": reason }),
            None => serde_json::json!({ "certificate": cert }),
        };

        let mut retry = retry();
        loop {
            retry.tick()?;

            let directory =
                Self::get_directory(&mut self.inner, &mut self.directory, &self.directory_url)?;
            let nonce = Self::nonce(&mut self.inner, directory)?;
            let request = account.post_request(&directory.data.revoke_cert, nonce, &data)?;
            match self.inner.run_request(request) {
                Ok(_response) => return Ok(()),
                Err(err) if err.is_bad_nonce() => continue,
                Err(err) => return Err(err.into()),
            }
        }
    }
}

fn parse_header(data: &[u8]) -> Option<(&str, &str)> {
    let colon = data.iter().position(|&b| b == b':')?;

    let name = std::str::from_utf8(&data[..colon]).ok()?;

    let value = &data[(colon + 1)..];
    let value_start = value.iter().position(|&b| !b.is_ascii_whitespace())?;
    let value = std::str::from_utf8(&value[value_start..]).ok()?;

    Some((name.trim(), value.trim()))
}

/// bad nonce retry count helper
struct Retry(usize);

const fn retry() -> Retry {
    Retry(0)
}

impl Retry {
    fn tick(&mut self) -> Result<(), Error> {
        if self.0 >= 3 {
            bail!("kept getting a badNonce error!");
        }
        self.0 += 1;
        Ok(())
    }
}
