//! ACME Account management and creation. The [`Account`] type also contains most of the ACME API
//! entry point helpers.

use std::collections::HashMap;
use std::convert::TryFrom;

use openssl::pkey::{PKey, Private};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::authorization::{Authorization, GetAuthorization};
use crate::b64u;
use crate::directory::Directory;
use crate::jws::Jws;
use crate::key::PublicKey;
use crate::order::{NewOrder, Order, OrderData};
use crate::request::Request;
use crate::Error;

/// An ACME Account.
///
/// This contains the location URL, the account data and the private key for an account.
/// This can directly be serialized via serde to persist the account.
///
/// In order to register a new account with an ACME provider, see the [`Account::creator`] method.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Account location URL.
    pub location: String,

    /// Acme account data.
    pub data: AccountData,

    /// base64url encoded PEM formatted private key.
    pub private_key: String,
}

impl Account {
    /// Rebuild an account from its components.
    pub fn from_parts(location: String, private_key: String, data: AccountData) -> Self {
        Self {
            location,
            data,
            private_key,
        }
    }

    /// Builds an [`AccountCreator`]. This handles creation of the private key and account data as
    /// well as handling the response sent by the server for the registration request.
    pub fn creator() -> AccountCreator {
        AccountCreator::default()
    }

    /// Place a new order. This will build a [`NewOrder`] representing an in flight order creation
    /// request.
    ///
    /// The returned `NewOrder`'s `request` option is *guaranteed* to be `Some(Request)`.
    pub fn new_order(
        &self,
        order: &OrderData,
        directory: &Directory,
        nonce: &str,
    ) -> Result<NewOrder, Error> {
        let key = PKey::private_key_from_pem(self.private_key.as_bytes())?;

        if order.identifiers.is_empty() {
            return Err(Error::EmptyOrder);
        }

        let url = directory.new_order_url();
        let body = serde_json::to_string(&Jws::new(
            &key,
            Some(self.location.clone()),
            url.to_owned(),
            nonce.to_owned(),
            order,
        )?)?;

        let request = Request {
            url: url.to_owned(),
            method: "POST",
            content_type: crate::request::JSON_CONTENT_TYPE,
            body,
            expected: crate::request::CREATED,
        };

        Ok(NewOrder::new(request))
    }

    /// Prepare a "POST-as-GET" request to fetch data. Low level helper.
    pub fn get_request(&self, url: &str, nonce: &str) -> Result<Request, Error> {
        let key = PKey::private_key_from_pem(self.private_key.as_bytes())?;
        let body = serde_json::to_string(&Jws::new_full(
            &key,
            Some(self.location.clone()),
            url.to_owned(),
            nonce.to_owned(),
            String::new(),
        )?)?;

        Ok(Request {
            url: url.to_owned(),
            method: "POST",
            content_type: crate::request::JSON_CONTENT_TYPE,
            body,
            expected: 200,
        })
    }

    /// Prepare a JSON POST request. Low level helper.
    pub fn post_request<T: Serialize>(
        &self,
        url: &str,
        nonce: &str,
        data: &T,
    ) -> Result<Request, Error> {
        let key = PKey::private_key_from_pem(self.private_key.as_bytes())?;
        let body = serde_json::to_string(&Jws::new(
            &key,
            Some(self.location.clone()),
            url.to_owned(),
            nonce.to_owned(),
            data,
        )?)?;

        Ok(Request {
            url: url.to_owned(),
            method: "POST",
            content_type: crate::request::JSON_CONTENT_TYPE,
            body,
            expected: 200,
        })
    }

    /// Prepare a JSON POST request.
    fn post_request_raw_payload(
        &self,
        url: &str,
        nonce: &str,
        payload: String,
    ) -> Result<Request, Error> {
        let key = PKey::private_key_from_pem(self.private_key.as_bytes())?;
        let body = serde_json::to_string(&Jws::new_full(
            &key,
            Some(self.location.clone()),
            url.to_owned(),
            nonce.to_owned(),
            payload,
        )?)?;

        Ok(Request {
            url: url.to_owned(),
            method: "POST",
            content_type: crate::request::JSON_CONTENT_TYPE,
            body,
            expected: 200,
        })
    }

    /// Get the "key authorization" for a token.
    pub fn key_authorization(&self, token: &str) -> Result<String, Error> {
        let key = PKey::private_key_from_pem(self.private_key.as_bytes())?;
        let thumbprint = PublicKey::try_from(&*key)?.thumbprint()?;
        Ok(format!("{}.{}", token, thumbprint))
    }

    /// Get the TXT field value for a dns-01 token. This is the base64url encoded sha256 digest of
    /// the key authorization value.
    pub fn dns_01_txt_value(&self, token: &str) -> Result<String, Error> {
        let key_authorization = self.key_authorization(token)?;
        let digest = openssl::sha::sha256(key_authorization.as_bytes());
        Ok(b64u::encode(&digest))
    }

    /// Prepare a request to update account data.
    ///
    /// This is a rather low level interface. You should know what you're doing.
    pub fn update_account_request<T: Serialize>(
        &self,
        nonce: &str,
        data: &T,
    ) -> Result<Request, Error> {
        self.post_request(&self.location, nonce, data)
    }

    /// Prepare a request to deactivate this account.
    pub fn deactivate_account_request<T: Serialize>(&self, nonce: &str) -> Result<Request, Error> {
        self.post_request_raw_payload(
            &self.location,
            nonce,
            r#"{"status":"deactivated"}"#.to_string(),
        )
    }

    /// Prepare a request to query an Authorization for an Order.
    ///
    /// Returns `Ok(None)` if `auth_index` is out of out of range. You can query the number of
    /// authorizations from via [`Order::authorization_len`] or by manually inspecting its
    /// `.data.authorization` vector.
    pub fn get_authorization(
        &self,
        order: &Order,
        auth_index: usize,
        nonce: &str,
    ) -> Result<Option<GetAuthorization>, Error> {
        match order.authorization(auth_index) {
            None => Ok(None),
            Some(url) => Ok(Some(GetAuthorization::new(self.get_request(url, nonce)?))),
        }
    }

    /// Prepare a request to validate a Challenge from an Authorization.
    ///
    /// Returns `Ok(None)` if `challenge_index` is out of out of range. The challenge count is
    /// available by inspecting the [`Authorization::challenges`] vector.
    ///
    /// This returns a raw `Request` since validation takes some time and the `Authorization`
    /// object has to be re-queried and its `status` inspected.
    pub fn validate_challenge(
        &self,
        authorization: &Authorization,
        challenge_index: usize,
        nonce: &str,
    ) -> Result<Option<Request>, Error> {
        match authorization.challenges.get(challenge_index) {
            None => Ok(None),
            Some(challenge) => self
                .post_request_raw_payload(&challenge.url, nonce, "{}".to_string())
                .map(Some),
        }
    }

    /// Prepare a request to revoke a certificate.
    ///
    /// The certificate can be either PEM or DER formatted.
    ///
    /// Note that this uses the account's key for authorization.
    ///
    /// Revocation using a certificate's private key is not yet implemented.
    pub fn revoke_certificate(
        &self,
        certificate: &[u8],
        reason: Option<u32>,
    ) -> Result<CertificateRevocation, Error> {
        let cert = if certificate.starts_with(b"-----BEGIN CERTIFICATE-----") {
            b64u::encode(&openssl::x509::X509::from_pem(certificate)?.to_der()?)
        } else {
            b64u::encode(certificate)
        };

        let data = match reason {
            Some(reason) => serde_json::json!({ "certificate": cert, "reason": reason }),
            None => serde_json::json!({ "certificate": cert }),
        };

        Ok(CertificateRevocation {
            account: self,
            data,
        })
    }
}

/// Certificate revocation involves converting the certificate to base64url encoded DER and then
/// embedding it in a json structure. Since we also need a nonce and possibly retry the request if
/// a `BadNonce` error happens, this caches the converted data for efficiency.
pub struct CertificateRevocation<'a> {
    account: &'a Account,
    data: Value,
}

impl CertificateRevocation<'_> {
    /// Create the revocation request using the specified nonce for the given directory.
    pub fn request(&self, directory: &Directory, nonce: &str) -> Result<Request, Error> {
        self.account
            .post_request(&directory.data.revoke_cert, nonce, &self.data)
    }
}

/// Status of an ACME account.
#[derive(Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountStatus {
    /// This is not part of the ACME API, but a temporary marker for us until the ACME provider
    /// tells us the account's real status.
    #[serde(rename = "<invalid>")]
    New,

    /// Means the account is valid and can be used.
    Valid,

    /// The account has been deactivated by its user and cannot be used anymore.
    Deactivated,

    /// The account has been revoked by the server and cannot be used anymore.
    Revoked,
}

impl AccountStatus {
    #[inline]
    fn new() -> Self {
        AccountStatus::New
    }

    #[inline]
    fn is_new(&self) -> bool {
        *self == AccountStatus::New
    }
}

/// ACME Account data. This is the part of the account returned from and possibly sent to the ACME
/// provider. Some fields may be uptdated by the user via a request to the account location, others
/// may not be changed.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountData {
    /// The current account status.
    #[serde(
        skip_serializing_if = "AccountStatus::is_new",
        default = "AccountStatus::new"
    )]
    pub status: AccountStatus,

    /// URLs to currently pending orders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orders: Option<String>,

    /// The acccount's contact info.
    ///
    /// This usually contains a `"mailto:<email address>"` entry but may also contain some other
    /// data if the server accepts it.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub contact: Vec<String>,

    /// Indicated whether the user agreed to the ACME provider's terms of service.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms_of_service_agreed: Option<bool>,

    /// External account information. This is currently not directly supported in any way and only
    /// stored to completeness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_account_binding: Option<Value>,

    /// This is only used by the client when querying an account.
    #[serde(default = "default_true", skip_serializing_if = "is_false")]
    pub only_return_existing: bool,

    /// Stores unknown fields if there are any.
    #[serde(flatten, default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, Value>,
}

#[inline]
fn default_true() -> bool {
    true
}

#[inline]
fn is_false(b: &bool) -> bool {
    !*b
}

/// Helper to create an account.
///
/// This is used to generate a private key and set the contact info for the account. Afterwards the
/// creation request can be created via the [`request`](AccountCreator::request()) method, giving
/// it a nonce and a directory.  This can be repeated, if necessary, like when the nonce fails.
///
/// When the server sends a succesful response, it should be passed to the
/// [`response`](AccountCreator::response()) method to finish the creation of an [`Account`] which
/// can then be persisted.
#[derive(Default)]
#[must_use = "when creating an account you must pass the response to AccountCreator::response()!"]
pub struct AccountCreator {
    contact: Vec<String>,
    terms_of_service_agreed: bool,
    key: Option<PKey<Private>>,
}

impl AccountCreator {
    /// Replace the contact infor with the provided ACME compatible data.
    pub fn set_contacts(mut self, contact: Vec<String>) -> Self {
        self.contact = contact;
        self
    }

    /// Append a contact string.
    pub fn contact(mut self, contact: String) -> Self {
        self.contact.push(contact);
        self
    }

    /// Append an email address to the contact list.
    pub fn email(self, email: String) -> Self {
        self.contact(format!("mailto:{}", email))
    }

    /// Change whether the account agrees to the terms of service. Use the directory's or client's
    /// `terms_of_service_url()` method to present the user with the Terms of Service.
    pub fn agree_to_tos(mut self, agree: bool) -> Self {
        self.terms_of_service_agreed = agree;
        self
    }

    /// Generate a new RSA key of the specified key size.
    pub fn generate_rsa_key(self, bits: u32) -> Result<Self, Error> {
        let key = openssl::rsa::Rsa::generate(bits)?;
        Ok(self.with_key(PKey::from_rsa(key)?))
    }

    /// Generate a new P-256 EC key.
    pub fn generate_ec_key(self) -> Result<Self, Error> {
        let key = openssl::ec::EcKey::generate(
            openssl::ec::EcGroup::from_curve_name(openssl::nid::Nid::X9_62_PRIME256V1)?.as_ref(),
        )?;
        Ok(self.with_key(PKey::from_ec_key(key)?))
    }

    /// Use an existing key. Note that only RSA and EC keys using the `P-256` curve are currently
    /// supported, however, this will not be checked at this point.
    pub fn with_key(mut self, key: PKey<Private>) -> Self {
        self.key = Some(key);
        self
    }

    /// Prepare a HTTP request to create this account.
    ///
    /// Changes to the user data made after this will have no effect on the account generated with
    /// the resulting request.
    /// Changing the private key between using the request and passing the response to
    /// [`response`](AccountCreator::response()) will render the account unusable!
    pub fn request(&self, directory: &Directory, nonce: &str) -> Result<Request, Error> {
        let key = self.key.as_deref().ok_or(Error::MissingKey)?;

        let data = AccountData {
            orders: None,
            status: AccountStatus::New,
            contact: self.contact.clone(),
            terms_of_service_agreed: if self.terms_of_service_agreed {
                Some(true)
            } else {
                None
            },
            external_account_binding: None,
            only_return_existing: false,
            extra: HashMap::new(),
        };

        let url = directory.new_account_url();
        let body = serde_json::to_string(&Jws::new(
            key,
            None,
            url.to_owned(),
            nonce.to_owned(),
            &data,
        )?)?;

        Ok(Request {
            url: url.to_owned(),
            method: "POST",
            content_type: crate::request::JSON_CONTENT_TYPE,
            body,
            expected: crate::request::CREATED,
        })
    }

    /// After issuing the request from [`request()`](AccountCreator::request()), the response's
    /// `Location` header and body must be passed to this for verification and to create an account
    /// which is to be persisted!
    pub fn response(self, location_header: String, response_body: &[u8]) -> Result<Account, Error> {
        let private_key = self
            .key
            .ok_or(Error::MissingKey)?
            .private_key_to_pem_pkcs8()?;
        let private_key = String::from_utf8(private_key).map_err(|_| {
            Error::Custom("PEM key contained illegal non-utf-8 characters".to_string())
        })?;

        Ok(Account {
            location: location_header,
            data: serde_json::from_slice(response_body)
                .map_err(|err| Error::BadAccountData(err.to_string()))?,
            private_key,
        })
    }
}
