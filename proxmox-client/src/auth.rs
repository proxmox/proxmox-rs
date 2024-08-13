use crate::Authentication;

/// How the client is logged in to the remote.
pub enum AuthenticationKind {
    /// With an API Ticket.
    Ticket(Authentication),

    /// With a token.
    Token(Token),
}

impl From<Authentication> for AuthenticationKind {
    fn from(auth: Authentication) -> Self {
        Self::Ticket(auth)
    }
}

impl From<Token> for AuthenticationKind {
    fn from(auth: Token) -> Self {
        Self::Token(auth)
    }
}

/// Data used to log in with a token.
pub struct Token {
    /// The userid.
    pub userid: String,

    /// The api token name (usually the product abbreviation).
    pub prefix: String,

    /// The api token's value.
    pub value: String,

    /// The separator for userid & value, due to inconsistencies between perl and rust based
    /// products.
    /// FIXME: Make one of them work for both types of products and remove this!
    pub perl_compat: bool,
}

impl Token {
    pub fn set_auth_headers(&self, request: http::request::Builder) -> http::request::Builder {
        let delim = if self.perl_compat { '=' } else { ':' };
        request.header(
            http::header::AUTHORIZATION,
            format!("{}={}{delim}{}", self.prefix, self.userid, self.value),
        )
    }
}
