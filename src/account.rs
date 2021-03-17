use std::collections::HashMap;
use std::convert::TryFrom;

use openssl::pkey::{PKey, Private};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::b64u;
use crate::directory::Directory;
use crate::jws::Jws;
use crate::key::PublicKey;
use crate::order::{NewOrder, OrderData};
use crate::request::Request;
use crate::Error;

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
    pub fn from_parts(location: String, private_key: String, data: AccountData) -> Self {
        Self {
            location,
            private_key,
            data,
        }
    }

    pub fn creator() -> AccountCreator {
        AccountCreator::default()
    }

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

    /// Prepare a "POST-as-GET" request to fetch data.
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

    /// Prepare a JSON POST request.
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
}

#[derive(Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountStatus {
    #[serde(rename = "<invalid>")]
    New,
    Valid,
    Deactivated,
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

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountData {
    #[serde(
        skip_serializing_if = "AccountStatus::is_new",
        default = "AccountStatus::new"
    )]
    pub status: AccountStatus,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub orders: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub contact: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms_of_service_agreed: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_account_binding: Option<Value>,

    #[serde(default = "default_true", skip_serializing_if = "is_false")]
    pub only_return_existing: bool,

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
    /// [`response()`] will render the account unusable!
    pub fn request(&self, directory: &Directory, nonce: &str) -> Result<Request, Error> {
        let key = self.key.as_deref().ok_or_else(|| Error::MissingKey)?;

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

    /// After issuing the request from [`request()`], the response's `Location` header and body
    /// must be passed to this for verification and to create an account which is to be persisted!
    pub fn response(self, location_header: String, response_body: &[u8]) -> Result<Account, Error> {
        let private_key = self
            .key
            .ok_or(Error::MissingKey)?
            .private_key_to_pem_pkcs8()?;
        let private_key = String::from_utf8(private_key).map_err(|_| {
            Error::Custom(format!("PEM key contained illegal non-utf-8 characters"))
        })?;

        Ok(Account {
            location: location_header,
            data: serde_json::from_slice(response_body)
                .map_err(|err| Error::BadAccountData(err.to_string()))?,
            private_key,
        })
    }
}
