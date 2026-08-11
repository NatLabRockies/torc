use super::context_types::Push;
use axum::http::HeaderMap;
use axum::http::header;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::marker::PhantomData;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scopes {
    All,
    Some(BTreeSet<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authorization {
    pub(crate) subject: String,
    pub(crate) scopes: Scopes,
    pub(crate) issuer: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthData;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Basic {
    pub(crate) username: String,
    pub(crate) password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bearer(pub(crate) String);

pub trait RcBound {
    type Result;
    fn push(self, value: Option<Authorization>) -> Self::Result;
}

impl<T> RcBound for T
where
    T: Push<Option<Authorization>>,
{
    type Result = <T as Push<Option<Authorization>>>::Result;

    fn push(self, value: Option<Authorization>) -> Self::Result {
        Push::push(self, value)
    }
}

pub(crate) fn from_headers(headers: &HeaderMap) -> Option<Basic> {
    let header = headers.get(header::AUTHORIZATION)?;
    let header = header.to_str().ok()?;
    let encoded = header.strip_prefix("Basic ")?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let (username, password) = decoded.split_once(':')?;
    Some(Basic {
        username: username.to_string(),
        password: Some(password.to_string()),
    })
}

#[derive(Debug, Clone, Default)]
pub struct AllowAllAuthenticator<T, RC> {
    _inner: PhantomData<T>,
    _context: PhantomData<RC>,
}
