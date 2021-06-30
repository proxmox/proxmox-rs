use std::path::Path;

use anyhow::{format_err, Error};
use serde::{Deserialize, Serialize};

mod http_client;
pub use http_client::http_client;

mod auth_state;
pub use auth_state::*;


use openidconnect::{
    //curl::http_client,
    core::{
        CoreProviderMetadata,
        CoreClient,
        CoreIdTokenClaims,
        CoreIdTokenVerifier,
        CoreAuthenticationFlow,
        CoreAuthDisplay,
        CoreAuthPrompt,
    },
    PkceCodeChallenge,
    PkceCodeVerifier,
    AuthorizationCode,
    ClientId,
    ClientSecret,
    CsrfToken,
    IssuerUrl,
    Nonce,
    OAuth2TokenResponse,
    RedirectUrl,
    Scope,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct OpenIdConfig {
    pub issuer_url: String,
    pub client_id: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub client_key: Option<String>,
}

pub struct OpenIdAuthenticator {
    client: CoreClient,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PublicAuthState {
    pub csrf_token: CsrfToken,
    pub realm: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PrivateAuthState {
    pub csrf_token: CsrfToken,
    pub nonce: Nonce,
    pub pkce_verifier: PkceCodeVerifier,
    pub ctime: i64,
}

impl PrivateAuthState {

    pub fn new() -> Self {
        let nonce = Nonce::new_random();
        let csrf_token = CsrfToken::new_random();
        let (_pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        PrivateAuthState {
            csrf_token,
            nonce,
            pkce_verifier,
            ctime: proxmox::tools::time::epoch_i64(),
        }
    }

    pub fn pkce_verifier(&self) -> PkceCodeVerifier {
        // Note: PkceCodeVerifier does not impl. clone()
        PkceCodeVerifier::new(self.pkce_verifier.secret().to_string())
    }

    pub fn pkce_challenge(&self) -> PkceCodeChallenge {
        PkceCodeChallenge::from_code_verifier_sha256(&self.pkce_verifier)
    }

    pub fn public_state_string(&self, realm: String) -> Result<String, Error> {
        let pub_state = PublicAuthState {
            csrf_token: self.csrf_token.clone(),
            realm,
        };
        Ok(serde_json::to_string(&pub_state)?)
    }
}

impl OpenIdAuthenticator {

    pub fn discover(config: &OpenIdConfig, redirect_url: &str) -> Result<Self, Error> {

        let client_id = ClientId::new(config.client_id.clone());
        let client_key = config.client_key.clone().map(|key| ClientSecret::new(key));
        let issuer_url = IssuerUrl::new(config.issuer_url.clone())?;

        let provider_metadata = CoreProviderMetadata::discover(&issuer_url, http_client)?;

        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            client_id,
            client_key,
        ).set_redirect_uri(RedirectUrl::new(String::from(redirect_url))?);

        Ok(Self {
            client,
        })
    }

    pub fn authorize_url(&self, state_dir: &str, realm: &str) -> Result<String, Error> {

        let private_auth_state = PrivateAuthState::new();
        let public_auth_state = private_auth_state.public_state_string(realm.to_string())?;
        let nonce = private_auth_state.nonce.clone();

        store_auth_state(Path::new(state_dir), realm, &private_auth_state)?;

         // Generate the authorization URL to which we'll redirect the user.
        let (authorize_url, _csrf_state, _nonce) = self.client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                || CsrfToken::new(public_auth_state),
                || nonce,
            )
            .set_display(CoreAuthDisplay::Page)
            .add_prompt(CoreAuthPrompt::Login)
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .set_pkce_challenge(private_auth_state.pkce_challenge())
            .url();

        Ok(authorize_url.to_string())
    }

    pub fn verify_public_auth_state(
        state_dir: &str,
        state: &str,
    ) -> Result<(String, PrivateAuthState), Error> {
        verify_public_auth_state(Path::new(state_dir), state)
    }

    pub fn verify_authorization_code(
        &self,
        code: &str,
        private_auth_state: &PrivateAuthState,
    ) -> Result<CoreIdTokenClaims, Error> {

        let code = AuthorizationCode::new(code.to_string());
        // Exchange the code with a token.
        let token_response = self.client
            .exchange_code(code)
            .set_pkce_verifier(private_auth_state.pkce_verifier())
            .request(http_client)
            .map_err(|err| format_err!("Failed to contact token endpoint: {}", err))?;

        let id_token_verifier: CoreIdTokenVerifier = self.client.id_token_verifier();
        let id_token_claims: &CoreIdTokenClaims = token_response
            .extra_fields()
            .id_token()
            .expect("Server did not return an ID token")
            .claims(&id_token_verifier, &private_auth_state.nonce)
            .map_err(|err| format_err!("Failed to verify ID token: {}", err))?;

        Ok(id_token_claims.clone())
    }
}
