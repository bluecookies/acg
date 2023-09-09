use std::collections::HashMap;
use std::env::VarError;
use serde::Deserialize;
use serde_aux::field_attributes::deserialize_number_from_string;
use reqwest::header::HeaderValue;
use thiserror::Error;

#[derive(Deserialize, Debug)]
pub struct Token {
    pub(crate) token: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub(crate) port: u16,
}


#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TokenError {
    #[error("environment variable error: {1} {0}")]
    EnvVarError(VarError, &'static str),
    #[error("request error: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("json error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("no cookie")]
    NoCookieError,
    #[error("TLS backend could not be initialised")]
    TlsInitialiseError(reqwest::Error),
}

// TODO: pass in username/password, or saved cookie
pub async fn get_amq_token(cookie: Option<String>) -> Result<(Token, Option<String>), TokenError> {
    const SIGNIN_URL: &str = "https://animemusicquiz.com/signIn";
    const GET_TOKEN_URL: &str = "https://animemusicquiz.com/socketToken";

    let client = reqwest::ClientBuilder::new()
        // .use_rustls_tls()
        .build()
        .map_err(TokenError::TlsInitialiseError)?;

    let cookie = if let Some(c) = cookie.and_then(|s| HeaderValue::from_str(&s).ok()) {
        c
    } else {
        let username = std::env::var("USERNAME").map_err(|e| TokenError::EnvVarError(e, "USERNAME"))?;
        let password = std::env::var("PASSWORD").map_err(|e| TokenError::EnvVarError(e, "PASSWORD"))?;
        let params = HashMap::<_, _>::from_iter(IntoIterator::into_iter([
            ("username", username),
            ("password", password)
        ]));

        let res = client.post(SIGNIN_URL)
            .form(&params)
            .send()
            .await?
            .error_for_status()?;
        let headers = res.headers();
        let cookie = if let Some(c) = headers.get(reqwest::header::SET_COOKIE) {
            c.clone()
        } else {
            return Err(TokenError::NoCookieError);
        };
        cookie
    };

    // TODO: if saved cookie doesnt work log in again
    let text = client.get(GET_TOKEN_URL)
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await?
        .text()
        .await?;
    // Not logged in
    log::info!("token response: {}", &text);
    let token = serde_json::from_str(&text)?;

    let cookie = cookie.to_str().map(str::to_string).ok();
    Ok((token, cookie))
}