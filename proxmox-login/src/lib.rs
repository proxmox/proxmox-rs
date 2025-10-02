//! This package provides helpers for logging into the APIs of Proxmox products such as Proxmox VE
//! or Proxmox Backup.

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use serde::{Deserialize, Serialize};

pub mod api;
pub mod error;
pub mod tfa;
pub mod ticket;

const CONTENT_TYPE_JSON: &str = "application/json";

#[doc(inline)]
pub use ticket::{Authentication, Ticket};

use error::{ResponseError, TfaError, TicketError};

/// The header name for the CSRF prevention token.
pub const CSRF_HEADER_NAME: &str = "CSRFPreventionToken";

/// A request to be sent to the ticket API call.
///
/// Note that the body is always JSON (`application/json`) and request method is POST.
#[derive(Clone, Debug)]
pub struct Request {
    pub url: String,

    /// This is always `application/json`.
    pub content_type: &'static str,

    /// The `Content-length` header field.
    pub content_length: usize,

    /// The body.
    pub body: String,
}

/// Login or ticket renewal request builder.
///
/// This takes an API URL and either a valid ticket or a userid (name + real) and password in order
/// to create an HTTP [`Request`] to renew or create a new API ticket.
///
/// Note that for Proxmox VE versions up to including 7, a compatibility flag is required to
/// support Two-Factor-Authentication.
#[derive(Debug)]
pub struct Login {
    api_url: String,
    userid: String,
    password: Option<String>,
    pve_compat: bool,
}

fn normalize_url(mut api_url: String) -> String {
    api_url.truncate(api_url.trim_end_matches('/').len());
    api_url
}

fn check_ticket_userid(ticket_userid: &str, expected_userid: &str) -> Result<(), ResponseError> {
    if ticket_userid != expected_userid.trim_end_matches("@quarantine") {
        return Err("returned ticket contained unexpected userid".into());
    }
    Ok(())
}

impl Login {
    /// Prepare a request given an existing ticket string.
    pub fn renew(
        api_url: impl Into<String>,
        ticket: impl Into<String>,
    ) -> Result<Self, TicketError> {
        Ok(Self::renew_ticket(api_url, ticket.into().parse()?))
    }

    /// Switch to a different url on the same server.
    pub fn set_url(&mut self, api_url: impl Into<String>) {
        self.api_url = api_url.into();
    }

    /// Get the userid this request is for.
    pub fn userid(&self) -> &str {
        &self.userid
    }

    /// Get the API url this request is for.
    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    /// Prepare a request given an already parsed ticket.
    pub fn renew_ticket(api_url: impl Into<String>, ticket: Ticket) -> Self {
        Self {
            api_url: normalize_url(api_url.into()),
            pve_compat: ticket.product() == "PVE",
            userid: ticket.userid().to_string(),
            password: Some(ticket.into()),
        }
    }

    /// Prepare a request with the assumption that the context handles the ticket (usually in a
    /// browser via an HttpOnly cookie).
    pub fn renew_with_cookie(api_url: impl Into<String>, userid: impl Into<String>) -> Self {
        Self {
            api_url: normalize_url(api_url.into()),
            pve_compat: false,
            userid: userid.into(),
            password: None,
        }
    }

    /// Prepare a request given a userid and password.
    pub fn new(
        api_url: impl Into<String>,
        userid: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            api_url: normalize_url(api_url.into()),
            userid: userid.into(),
            password: Some(password.into()),
            pve_compat: false,
        }
    }

    /// Set the Proxmox VE compatibility parameter for Two-Factor-Authentication support.
    pub fn pve_compatibility(mut self, compatibility: bool) -> Self {
        self.pve_compat = compatibility;
        self
    }

    /// Create an HTTP [`Request`] from the current data.
    ///
    /// If the request returns a successful result, the response's body should be passed to the
    /// [`response`](Login::response) method in order to extract the validated ticket or
    /// Two-Factor-Authentication challenge.
    pub fn request(&self) -> Request {
        let request = api::CreateTicket {
            new_format: self.pve_compat.then_some(true),
            username: self.userid.clone(),
            password: self.password.clone(),
            ..Default::default()
        };

        let body = serde_json::to_string(&request).unwrap(); // this can never fail

        Request {
            url: format!("{}/api2/json/access/ticket", self.api_url),
            content_type: CONTENT_TYPE_JSON,
            content_length: body.len(),
            body,
        }
    }

    /// Parse the result body of a [`CreateTicket`](api::CreateTicket) API request.
    ///
    /// On success, this will either yield an [`Authentication`] or a [`SecondFactorChallenge`] if
    /// Two-Factor-Authentication is required.
    pub fn response<T: ?Sized + AsRef<[u8]>>(
        &self,
        body: &T,
    ) -> Result<TicketResult, ResponseError> {
        self.response_bytes(None, body.as_ref())
    }

    /// Parse the result body of a [`CreateTicket`](api::CreateTicket) API request taking into
    /// account potential tickets obtained via a `Set-Cookie` header.
    ///
    /// On success, this will either yield an [`Authentication`] or a [`SecondFactorChallenge`] if
    /// Two-Factor-Authentication is required.
    pub fn response_with_cookie_ticket<T: ?Sized + AsRef<[u8]>>(
        &self,
        cookie_ticket: Option<Ticket>,
        body: &T,
    ) -> Result<TicketResult, ResponseError> {
        self.response_bytes(cookie_ticket, body.as_ref())
    }

    fn response_bytes(
        &self,
        cookie_ticket: Option<Ticket>,
        body: &[u8],
    ) -> Result<TicketResult, ResponseError> {
        use ticket::TicketResponse;

        let response: api::ApiResponse<api::CreateTicketResponse> = serde_json::from_slice(body)?;
        let response = response.data.ok_or("missing response data")?;

        if response.username != self.userid {
            return Err("ticket response contained unexpected userid".into());
        }

        // if a ticket was provided via a cookie, use it like a normal ticket
        if let Some(ticket) = cookie_ticket {
            check_ticket_userid(ticket.userid(), &self.userid)?;
            return Ok(TicketResult::Full(
                self.authentication_for(ticket, response)?,
            ));
        }

        // old authentication flow where we needed to handle the ticket ourselves even in the
        // browser etc.
        let ticket: TicketResponse = match response.ticket {
            Some(ref ticket) => ticket.parse()?,
            None => {
                // `ticket_info` is set when the server sets the ticket via a HttpOnly cookie. this
                // also means we do not have access to the cookie itself which happens for example
                // in a browser. assume that the cookie is handled properly by the context
                // (browser) and don't worry about handling it ourselves.
                if let Some(ref ticket) = response.ticket_info {
                    let ticket = ticket.parse()?;
                    return Ok(TicketResult::HttpOnly(
                        self.authentication_for(ticket, response)?,
                    ));
                }

                return Err("no ticket information in response".into());
            }
        };

        Ok(match ticket {
            TicketResponse::Full(ticket) => {
                check_ticket_userid(ticket.userid(), &self.userid)?;
                TicketResult::Full(self.authentication_for(ticket, response)?)
            }

            TicketResponse::Tfa(ticket, challenge) => {
                TicketResult::TfaRequired(SecondFactorChallenge {
                    api_url: self.api_url.clone(),
                    pve_compat: self.pve_compat,
                    userid: response.username,
                    ticket,
                    challenge,
                })
            }
        })
    }

    fn authentication_for(
        &self,
        ticket: Ticket,
        response: api::CreateTicketResponse,
    ) -> Result<Authentication, ResponseError> {
        Ok(Authentication {
            csrfprevention_token: response
                .csrfprevention_token
                .ok_or("missing CSRFPreventionToken in ticket response")?,
            clustername: response.clustername,
            api_url: self.api_url.clone(),
            userid: response.username,
            ticket,
        })
    }
}

/// This is the result of a ticket call. It will either yield a final ticket, or a TFA challenge.
///
/// This is serializable in order to easily store it for later reuse.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TicketResult {
    /// The response contained a valid ticket.
    Full(Authentication),

    /// The response returned a Two-Factor-Authentication challenge.
    TfaRequired(SecondFactorChallenge),

    /// The response returned a valid ticket as an HttpOnly cookie.
    HttpOnly(Authentication),
}

/// A ticket call can returned a TFA challenge. The user should inspect the
/// [`challenge`](tfa::TfaChallenge) member and call one of the `respond_*` methods which will
/// yield a HTTP [`Request`] which should be used to finish the authentication.
///
/// Finally, the response should be passed to the [`response`](SecondFactorChallenge::response)
/// method to get the ticket.
///
/// This is serializable in order to easily store it for later reuse.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SecondFactorChallenge {
    api_url: String,
    pve_compat: bool,
    userid: String,
    ticket: String,
    pub challenge: tfa::TfaChallenge,
}

impl SecondFactorChallenge {
    /// Create a HTTP request responding to a Yubico OTP challenge.
    ///
    /// Errors with `TfaError::Unavailable` if Yubic OTP is not available.
    pub fn respond_yubico(&self, code: &str) -> Result<Request, TfaError> {
        if !self.challenge.yubico {
            Err(TfaError::Unavailable)
        } else {
            Ok(self.respond_raw(&format!("yubico:{code}")))
        }
    }

    /// Create a HTTP request responding with a TOTP value.
    ///
    /// Errors with `TfaError::Unavailable` if TOTP is not available.
    pub fn respond_totp(&self, code: &str) -> Result<Request, TfaError> {
        if !self.challenge.totp {
            Err(TfaError::Unavailable)
        } else {
            Ok(self.respond_raw(&format!("totp:{code}")))
        }
    }

    /// Create a HTTP request responding with a recovery code.
    ///
    /// Errors with `TfaError::Unavailable` if no recovery codes are available.
    pub fn respond_recovery(&self, code: &str) -> Result<Request, TfaError> {
        if !self.challenge.recovery.is_available() {
            Err(TfaError::Unavailable)
        } else {
            Ok(self.respond_raw(&format!("recovery:{code}")))
        }
    }

    #[cfg(feature = "webauthn")]
    /// Create a HTTP request responding with a FIDO2/webauthn result JSON string.
    ///
    /// Errors with `TfaError::Unavailable` if no webauthn challenge was available.
    pub fn respond_webauthn(&self, json_string: &str) -> Result<Request, TfaError> {
        if self.challenge.webauthn.is_none() {
            Err(TfaError::Unavailable)
        } else {
            Ok(self.respond_raw(&format!("webauthn:{json_string}")))
        }
    }

    /// Create a HTTP request using a raw response.
    ///
    /// A raw response is the response string prefixed with its challenge type and a colon.
    pub fn respond_raw(&self, data: &str) -> Request {
        let request = api::CreateTicket {
            new_format: self.pve_compat.then_some(true),
            username: self.userid.clone(),
            password: Some(data.to_string()),
            tfa_challenge: Some(self.ticket.clone()),
            ..Default::default()
        };

        let body = serde_json::to_string(&request).unwrap();

        Request {
            url: format!("{}/api2/json/access/ticket", self.api_url),
            content_type: CONTENT_TYPE_JSON,
            content_length: body.len(),
            body,
        }
    }

    /// Deal with the API's response object to extract the ticket.
    pub fn response<T: ?Sized + AsRef<[u8]>>(
        &self,
        body: &T,
    ) -> Result<Authentication, ResponseError> {
        self.response_bytes(None, body.as_ref())
    }

    /// Deal with the API's response object to extract the ticket either from a cookie or the
    /// response itself.
    pub fn response_with_cookie_ticket<T: ?Sized + AsRef<[u8]>>(
        &self,
        cookie_ticket: Option<Ticket>,
        body: &T,
    ) -> Result<Authentication, ResponseError> {
        self.response_bytes(cookie_ticket, body.as_ref())
    }

    fn response_bytes(
        &self,
        cookie_ticket: Option<Ticket>,
        body: &[u8],
    ) -> Result<Authentication, ResponseError> {
        let response: api::ApiResponse<api::CreateTicketResponse> = serde_json::from_slice(body)?;
        let response = response.data.ok_or("missing response data")?;

        if response.username != self.userid {
            return Err("ticket response contained unexpected userid".into());
        }

        // get the ticket from:
        // 1. the cookie if possible -> new HttpOnly authentication outside of the browser
        // 2. the `ticket` field -> old authentication flow where we handle the ticket ourselves
        // 3. if there is no `ticket` field, check if we have a `ticket_info` field -> new HttpOnly
        //    authentication inside of a browser (or similar context) that handles the ticket for us
        let ticket: Ticket = cookie_ticket
            .ok_or(ResponseError::from("no ticket in response"))
            .or_else(|e| {
                response
                    .ticket
                    .or(response.ticket_info)
                    .ok_or(e)
                    .and_then(|t| t.parse().map_err(|e: TicketError| e.into()))
            })?;

        check_ticket_userid(ticket.userid(), &self.userid)?;

        Ok(Authentication {
            ticket,
            csrfprevention_token: response
                .csrfprevention_token
                .ok_or("missing CSRFPreventionToken in ticket response")?,
            clustername: response.clustername,
            userid: response.username,
            api_url: self.api_url.clone(),
        })
    }
}
