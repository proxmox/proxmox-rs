//! Types for user handling.
//!
//! We have [`Username`]s, [`Realm`]s and [`Tokenname`]s. To uniquely identify a user/API token, they
//! must be combined into a [`Userid`] or [`Authid`].
//!
//! Since they're all string types, they're organized as follows:
//!
//! * [`Username`]: an owned user name. Internally a `String`.
//! * [`UsernameRef`]: a borrowed user name. Pairs with a `Username` the same way a `str` pairs
//!   with `String`, meaning you can only make references to it.
//! * [`Realm`]: an owned realm (`String` equivalent).
//! * [`RealmRef`]: a borrowed realm (`str` equivalent).
//! * [`Tokenname`]: an owned API token name (`String` equivalent)
//! * [`TokennameRef`]: a borrowed `Tokenname` (`str` equivalent).
//! * [`Userid`]: an owned user id (`"user@realm"`).
//! * [`Authid`]: an owned Authentication ID (a `Userid` with an optional `Tokenname`).
//!   Note that `Userid` and `Authid` do not have a separate borrowed type.
//!
//! Note that `Username`s are not unique, therefore they do not implement `Eq` and cannot be
//! compared directly. If a direct comparison is really required, they can be compared as strings
//! via the `as_str()` method. [`Realm`]s, [`Userid`]s and [`Authid`]s on the other hand can be
//! compared with each other, as in those cases the comparison has meaning.

use std::borrow::Borrow;
use std::fmt;
use std::sync::LazyLock;

use anyhow::{bail, format_err, Error};
use const_format::concatcp;
use serde::{Deserialize, Serialize};

use proxmox_schema::{
    api, const_regex, ApiStringFormat, ApiType, Schema, StringSchema, UpdaterType,
};

use proxmox_schema::api_types::SAFE_ID_REGEX_STR;

// we only allow a limited set of characters
// colon is not allowed, because we store usernames in
// colon separated lists)!
// slash is not allowed because it is used as pve API delimiter
// also see "man useradd"
pub const USER_NAME_REGEX_STR: &str = r"(?:[^\s:/[:cntrl:]]+)";

pub const GROUP_NAME_REGEX_STR: &str = USER_NAME_REGEX_STR;

pub const TOKEN_NAME_REGEX_STR: &str = SAFE_ID_REGEX_STR;

pub const USER_ID_REGEX_STR: &str = concatcp!(USER_NAME_REGEX_STR, r"@", SAFE_ID_REGEX_STR);

pub const APITOKEN_ID_REGEX_STR: &str = concatcp!(USER_ID_REGEX_STR, r"!", TOKEN_NAME_REGEX_STR);

const_regex! {
    pub PROXMOX_USER_NAME_REGEX = concatcp!(r"^",  USER_NAME_REGEX_STR, r"$");
    pub PROXMOX_TOKEN_NAME_REGEX = concatcp!(r"^", TOKEN_NAME_REGEX_STR, r"$");
    pub PROXMOX_USER_ID_REGEX = concatcp!(r"^",  USER_ID_REGEX_STR, r"$");
    pub PROXMOX_APITOKEN_ID_REGEX = concatcp!(r"^", APITOKEN_ID_REGEX_STR, r"$");
    pub PROXMOX_AUTH_ID_REGEX = concatcp!(r"^", r"(?:", USER_ID_REGEX_STR, r"|", APITOKEN_ID_REGEX_STR, r")$");
    pub PROXMOX_GROUP_ID_REGEX = concatcp!(r"^",  GROUP_NAME_REGEX_STR, r"$");
}

pub const PROXMOX_USER_NAME_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&PROXMOX_USER_NAME_REGEX);
pub const PROXMOX_TOKEN_NAME_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&PROXMOX_TOKEN_NAME_REGEX);

pub const PROXMOX_USER_ID_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&PROXMOX_USER_ID_REGEX);
pub const PROXMOX_TOKEN_ID_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&PROXMOX_APITOKEN_ID_REGEX);
pub const PROXMOX_AUTH_ID_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&PROXMOX_AUTH_ID_REGEX);

pub const PROXMOX_TOKEN_ID_SCHEMA: Schema = StringSchema::new("API Token ID")
    .format(&PROXMOX_TOKEN_ID_FORMAT)
    .min_length(3)
    .max_length(64)
    .schema();

pub const PROXMOX_TOKEN_NAME_SCHEMA: Schema = StringSchema::new("API Token name")
    .format(&PROXMOX_TOKEN_NAME_FORMAT)
    .min_length(3)
    .max_length(64)
    .schema();

pub const PROXMOX_GROUP_ID_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&PROXMOX_GROUP_ID_REGEX);

pub const PROXMOX_GROUP_ID_SCHEMA: Schema = StringSchema::new("Group ID")
    .format(&PROXMOX_GROUP_ID_FORMAT)
    .min_length(3)
    .max_length(64)
    .schema();

pub const PROXMOX_AUTH_REALM_STRING_SCHEMA: StringSchema =
    StringSchema::new("Authentication domain ID")
        .format(&proxmox_schema::api_types::SAFE_ID_FORMAT)
        .min_length(3)
        .max_length(32);
pub const PROXMOX_AUTH_REALM_SCHEMA: Schema = PROXMOX_AUTH_REALM_STRING_SCHEMA.schema();

#[api(
    type: String,
    format: &PROXMOX_USER_NAME_FORMAT,
    min_length: 1,
)]
/// The user name part of a user id.
///
/// This alone does NOT uniquely identify the user and therefore does not implement `Eq`. In order
/// to compare user names directly, they need to be explicitly compared as strings by calling
/// `.as_str()`.
///
/// ```compile_fail
/// # use proxmox_auth_api::types::Username;
/// fn test(a: Username, b: Username) -> bool {
///     a == b // illegal and does not compile
/// }
/// ```
#[derive(Clone, Debug, Hash, Deserialize, Serialize)]
pub struct Username(String);

/// A reference to a user name part of a user id. This alone does NOT uniquely identify the user.
///
/// This is like a `str` to the `String` of a [`Username`].
#[derive(Debug, Hash)]
pub struct UsernameRef(str);

impl UsernameRef {
    fn new(s: &str) -> &Self {
        unsafe { &*(s as *const str as *const UsernameRef) }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Username {
    type Target = UsernameRef;

    fn deref(&self) -> &UsernameRef {
        self.borrow()
    }
}

impl Borrow<UsernameRef> for Username {
    fn borrow(&self) -> &UsernameRef {
        UsernameRef::new(self.0.as_str())
    }
}

impl AsRef<UsernameRef> for Username {
    fn as_ref(&self) -> &UsernameRef {
        self.borrow()
    }
}

impl ToOwned for UsernameRef {
    type Owned = Username;

    fn to_owned(&self) -> Self::Owned {
        Username(self.0.to_owned())
    }
}

impl TryFrom<String> for Username {
    type Error = Error;

    fn try_from(s: String) -> Result<Self, Error> {
        if !PROXMOX_USER_NAME_REGEX.is_match(&s) {
            bail!("invalid user name");
        }

        Ok(Self(s))
    }
}

impl<'a> TryFrom<&'a str> for &'a UsernameRef {
    type Error = Error;

    fn try_from(s: &'a str) -> Result<&'a UsernameRef, Error> {
        if !PROXMOX_USER_NAME_REGEX.is_match(s) {
            bail!("invalid name in user id");
        }

        Ok(UsernameRef::new(s))
    }
}

#[api(schema: PROXMOX_AUTH_REALM_SCHEMA)]
/// An authentication realm.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct Realm(String);

/// A reference to an authentication realm.
///
/// This is like a `str` to the `String` of a `Realm`.
#[derive(Debug, Hash, Eq, PartialEq)]
pub struct RealmRef(str);

impl RealmRef {
    fn new(s: &str) -> &Self {
        unsafe { &*(s as *const str as *const RealmRef) }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Realm {
    type Target = RealmRef;

    fn deref(&self) -> &RealmRef {
        self.borrow()
    }
}

impl Borrow<RealmRef> for Realm {
    fn borrow(&self) -> &RealmRef {
        RealmRef::new(self.0.as_str())
    }
}

impl AsRef<RealmRef> for Realm {
    fn as_ref(&self) -> &RealmRef {
        self.borrow()
    }
}

impl ToOwned for RealmRef {
    type Owned = Realm;

    fn to_owned(&self) -> Self::Owned {
        Realm(self.0.to_owned())
    }
}

impl TryFrom<String> for Realm {
    type Error = Error;

    fn try_from(s: String) -> Result<Self, Error> {
        PROXMOX_AUTH_REALM_STRING_SCHEMA
            .check_constraints(&s)
            .map_err(|_| format_err!("invalid realm"))?;

        Ok(Self(s))
    }
}

impl<'a> TryFrom<&'a str> for &'a RealmRef {
    type Error = Error;

    fn try_from(s: &'a str) -> Result<&'a RealmRef, Error> {
        PROXMOX_AUTH_REALM_STRING_SCHEMA
            .check_constraints(s)
            .map_err(|_| format_err!("invalid realm"))?;

        Ok(RealmRef::new(s))
    }
}

impl PartialEq<str> for Realm {
    fn eq(&self, rhs: &str) -> bool {
        self.0 == rhs
    }
}

impl PartialEq<&str> for Realm {
    fn eq(&self, rhs: &&str) -> bool {
        self.0 == *rhs
    }
}

impl PartialEq<str> for RealmRef {
    fn eq(&self, rhs: &str) -> bool {
        self.0 == *rhs
    }
}

impl PartialEq<&str> for RealmRef {
    fn eq(&self, rhs: &&str) -> bool {
        self.0 == **rhs
    }
}

impl PartialEq<RealmRef> for Realm {
    fn eq(&self, rhs: &RealmRef) -> bool {
        self.0 == rhs.0
    }
}

impl PartialEq<Realm> for RealmRef {
    fn eq(&self, rhs: &Realm) -> bool {
        self.0 == rhs.0
    }
}

impl PartialEq<Realm> for &RealmRef {
    fn eq(&self, rhs: &Realm) -> bool {
        self.0 == rhs.0
    }
}

impl fmt::Display for Realm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Display for RealmRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[api(
    type: String,
    format: &PROXMOX_TOKEN_NAME_FORMAT,
)]
/// The token ID part of an API token authentication id.
///
/// This alone does NOT uniquely identify the API token - use a full `Authid` for such use cases.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq, Deserialize, Serialize)]
pub struct Tokenname(String);

/// A reference to a token name part of an authentication id. This alone does NOT uniquely identify
/// the user.
///
/// This is like a `str` to the `String` of a [`Tokenname`].
#[derive(Debug, Hash)]
pub struct TokennameRef(str);

#[doc(hidden)]
/// ```compile_fail
/// # use proxmox_auth_api::types::Username;
/// let a: Username = unsafe { std::mem::zeroed() };
/// let b: Username = unsafe { std::mem::zeroed() };
/// let _ = <Username as PartialEq>::eq(&a, &b);
/// ```
///
/// ```compile_fail
/// # use proxmox_auth_api::types::UsernameRef;
/// let a: &UsernameRef = unsafe { std::mem::zeroed() };
/// let b: &UsernameRef = unsafe { std::mem::zeroed() };
/// let _ = <&UsernameRef as PartialEq>::eq(a, b);
/// ```
///
/// ```compile_fail
/// # use proxmox_auth_api::types::TokennameRef;
/// let a: &TokennameRef = unsafe { std::mem::zeroed() };
/// let b: &TokennameRef = unsafe { std::mem::zeroed() };
/// let _ = <&TokennameRef as PartialEq>::eq(&a, &b);
/// ```
struct _AssertNoEqImpl;

impl TokennameRef {
    fn new(s: &str) -> &Self {
        unsafe { &*(s as *const str as *const TokennameRef) }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Tokenname {
    type Target = TokennameRef;

    fn deref(&self) -> &TokennameRef {
        self.borrow()
    }
}

impl Borrow<TokennameRef> for Tokenname {
    fn borrow(&self) -> &TokennameRef {
        TokennameRef::new(self.0.as_str())
    }
}

impl AsRef<TokennameRef> for Tokenname {
    fn as_ref(&self) -> &TokennameRef {
        self.borrow()
    }
}

impl ToOwned for TokennameRef {
    type Owned = Tokenname;

    fn to_owned(&self) -> Self::Owned {
        Tokenname(self.0.to_owned())
    }
}

impl TryFrom<String> for Tokenname {
    type Error = Error;

    fn try_from(s: String) -> Result<Self, Error> {
        if !PROXMOX_TOKEN_NAME_REGEX.is_match(&s) {
            bail!("invalid token name");
        }

        Ok(Self(s))
    }
}

impl<'a> TryFrom<&'a str> for &'a TokennameRef {
    type Error = Error;

    fn try_from(s: &'a str) -> Result<&'a TokennameRef, Error> {
        if !PROXMOX_TOKEN_NAME_REGEX.is_match(s) {
            bail!("invalid token name in user id");
        }

        Ok(TokennameRef::new(s))
    }
}

/// A complete user id consisting of a user name and a realm
#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, UpdaterType)]
pub struct Userid {
    data: String,
    name_len: usize,
}

impl ApiType for Userid {
    const API_SCHEMA: Schema = StringSchema::new("User ID")
        .format(&PROXMOX_USER_ID_FORMAT)
        .min_length(3)
        .max_length(64)
        .schema();
}

impl Userid {
    const fn new(data: String, name_len: usize) -> Self {
        Self { data, name_len }
    }

    pub fn name(&self) -> &UsernameRef {
        UsernameRef::new(&self.data[..self.name_len])
    }

    pub fn realm(&self) -> &RealmRef {
        RealmRef::new(&self.data[(self.name_len + 1)..])
    }

    pub fn as_str(&self) -> &str {
        &self.data
    }

    /// Get the "root@pam" user id.
    pub fn root_userid() -> &'static Self {
        &ROOT_USERID
    }
}

pub static ROOT_USERID: LazyLock<Userid> = LazyLock::new(|| Userid::new("root@pam".to_string(), 4));

impl From<Authid> for Userid {
    fn from(authid: Authid) -> Self {
        authid.user
    }
}

impl From<(Username, Realm)> for Userid {
    fn from(parts: (Username, Realm)) -> Self {
        Self::from((parts.0.as_ref(), parts.1.as_ref()))
    }
}

impl From<(&UsernameRef, &RealmRef)> for Userid {
    fn from(parts: (&UsernameRef, &RealmRef)) -> Self {
        let data = format!("{}@{}", parts.0.as_str(), parts.1.as_str());
        let name_len = parts.0.as_str().len();
        Self { data, name_len }
    }
}

impl fmt::Display for Userid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.data.fmt(f)
    }
}

impl std::str::FromStr for Userid {
    type Err = Error;

    fn from_str(id: &str) -> Result<Self, Error> {
        let name_len = id
            .as_bytes()
            .iter()
            .rposition(|&b| b == b'@')
            .ok_or_else(|| format_err!("not a valid user id"))?;

        let name = &id[..name_len];
        let realm = &id[(name_len + 1)..];

        if !PROXMOX_USER_NAME_REGEX.is_match(name) {
            bail!("invalid user name in user id");
        }

        PROXMOX_AUTH_REALM_STRING_SCHEMA
            .check_constraints(realm)
            .map_err(|_| format_err!("invalid realm in user id"))?;

        Ok(Self::from((UsernameRef::new(name), RealmRef::new(realm))))
    }
}

impl TryFrom<String> for Userid {
    type Error = Error;

    fn try_from(data: String) -> Result<Self, Error> {
        let name_len = data
            .as_bytes()
            .iter()
            .rposition(|&b| b == b'@')
            .ok_or_else(|| format_err!("not a valid user id"))?;

        if !PROXMOX_USER_NAME_REGEX.is_match(&data[..name_len]) {
            bail!("invalid user name in user id");
        }

        PROXMOX_AUTH_REALM_STRING_SCHEMA
            .check_constraints(&data[(name_len + 1)..])
            .map_err(|_| format_err!("invalid realm in user id"))?;

        Ok(Self { data, name_len })
    }
}

impl PartialEq<str> for Userid {
    fn eq(&self, rhs: &str) -> bool {
        self.data == *rhs
    }
}

impl PartialEq<&str> for Userid {
    fn eq(&self, rhs: &&str) -> bool {
        *self == **rhs
    }
}

impl PartialEq<String> for Userid {
    fn eq(&self, rhs: &String) -> bool {
        self == rhs.as_str()
    }
}

/// A complete authentication id consisting of a user id and an optional token name.
#[derive(Clone, Debug, Eq, PartialEq, Hash, UpdaterType, Ord, PartialOrd)]
pub struct Authid {
    user: Userid,
    tokenname: Option<Tokenname>,
}

impl ApiType for Authid {
    const API_SCHEMA: Schema = StringSchema::new("Authentication ID")
        .format(&PROXMOX_AUTH_ID_FORMAT)
        .min_length(3)
        .max_length(64)
        .schema();
}

impl Authid {
    const fn new(user: Userid, tokenname: Option<Tokenname>) -> Self {
        Self { user, tokenname }
    }

    pub fn user(&self) -> &Userid {
        &self.user
    }

    pub fn is_token(&self) -> bool {
        self.tokenname.is_some()
    }

    pub fn tokenname(&self) -> Option<&TokennameRef> {
        self.tokenname.as_deref()
    }

    /// Get the "root@pam" auth id.
    pub fn root_auth_id() -> &'static Self {
        &ROOT_AUTHID
    }
}

pub static ROOT_AUTHID: LazyLock<Authid> =
    LazyLock::new(|| Authid::from(Userid::new("root@pam".to_string(), 4)));

impl From<Userid> for Authid {
    fn from(parts: Userid) -> Self {
        Self::new(parts, None)
    }
}

impl From<(Userid, Option<Tokenname>)> for Authid {
    fn from(parts: (Userid, Option<Tokenname>)) -> Self {
        Self::new(parts.0, parts.1)
    }
}

impl fmt::Display for Authid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.tokenname {
            Some(token) => write!(f, "{}!{}", self.user, token.as_str()),
            None => self.user.fmt(f),
        }
    }
}

impl std::str::FromStr for Authid {
    type Err = Error;

    fn from_str(id: &str) -> Result<Self, Error> {
        let name_len = id
            .as_bytes()
            .iter()
            .rposition(|&b| b == b'@')
            .ok_or_else(|| format_err!("not a valid user id"))?;

        let realm_end = id
            .as_bytes()
            .iter()
            .rposition(|&b| b == b'!')
            .map(|pos| if pos < name_len { id.len() } else { pos })
            .unwrap_or_else(|| id.len());

        if realm_end == id.len() - 1 {
            bail!("empty token name in userid");
        }

        let user = Userid::from_str(&id[..realm_end])?;

        if id.len() > realm_end {
            let token = Tokenname::try_from(id[(realm_end + 1)..].to_string())?;
            Ok(Self::new(user, Some(token)))
        } else {
            Ok(Self::new(user, None))
        }
    }
}

impl TryFrom<String> for Authid {
    type Error = Error;

    fn try_from(mut data: String) -> Result<Self, Error> {
        let name_len = data
            .as_bytes()
            .iter()
            .rposition(|&b| b == b'@')
            .ok_or_else(|| format_err!("not a valid user id"))?;

        let realm_end = data
            .as_bytes()
            .iter()
            .rposition(|&b| b == b'!')
            .map(|pos| if pos < name_len { data.len() } else { pos })
            .unwrap_or_else(|| data.len());

        if realm_end == data.len() - 1 {
            bail!("empty token name in userid");
        }

        let tokenname = if data.len() > realm_end {
            Some(Tokenname::try_from(data[(realm_end + 1)..].to_string())?)
        } else {
            None
        };

        data.truncate(realm_end);

        let user: Userid = data.parse()?;

        Ok(Self { user, tokenname })
    }
}

#[api]
/// The parameter object for creating new ticket.
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateTicket {
    /// User name
    pub username: Userid,

    /// The secret password. This can also be a valid ticket. Only optional if the ticket is
    /// provided in a cookie header and only if the endpoint supports this.
    #[serde(default)]
    pub password: Option<String>,

    /// Verify ticket, and check if user have access 'privs' on 'path'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Verify ticket, and check if user have access 'privs' on 'path'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privs: Option<String>,

    /// Port for verifying terminal tickets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    /// The signed TFA challenge string the user wants to respond to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "tfa-challenge")]
    pub tfa_challenge: Option<String>,
}

#[api]
/// The API response for a ticket call.
#[derive(Debug, Deserialize, Serialize)]
pub struct CreateTicketResponse {
    /// The CSRF prevention token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "CSRFPreventionToken")]
    pub csrfprevention_token: Option<String>,

    /// The ticket as is supposed to be used in the authentication header. Not provided here if the
    /// endpoint uses HttpOnly cookies to supply the actual ticket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticket: Option<String>,

    /// Like a full ticket, except the signature is missing. Useful in HttpOnly-contexts
    /// (browsers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ticket-info")]
    pub ticket_info: Option<String>,

    /// The userid.
    pub username: Userid,
}

impl CreateTicketResponse {
    pub fn new(username: Userid) -> Self {
        Self {
            csrfprevention_token: None,
            ticket: None,
            ticket_info: None,
            username,
        }
    }
}

#[test]
fn test_token_id() {
    let userid: Userid = "test@pam".parse().expect("parsing Userid failed");
    assert_eq!(userid.name().as_str(), "test");
    assert_eq!(userid.realm(), "pam");
    assert_eq!(userid, "test@pam");

    let auth_id: Authid = "test@pam".parse().expect("parsing user Authid failed");
    assert_eq!(auth_id.to_string(), "test@pam".to_string());
    assert!(!auth_id.is_token());

    assert_eq!(auth_id.user(), &userid);

    let user_auth_id = Authid::from(userid.clone());
    assert_eq!(user_auth_id, auth_id);
    assert!(!user_auth_id.is_token());

    let auth_id: Authid = "test@pam!bar".parse().expect("parsing token Authid failed");
    let token_userid = auth_id.user();
    assert_eq!(&userid, token_userid);
    assert!(auth_id.is_token());
    assert_eq!(
        auth_id.tokenname().expect("Token has tokenname").as_str(),
        TokennameRef::new("bar").as_str()
    );
    assert_eq!(auth_id.to_string(), "test@pam!bar".to_string());
}

serde_plain::derive_deserialize_from_fromstr!(Userid, "valid user id");
serde_plain::derive_serialize_from_display!(Userid);

serde_plain::derive_deserialize_from_fromstr!(Authid, "valid user id or token id");
serde_plain::derive_serialize_from_display!(Authid);
