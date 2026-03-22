use axum::http::Request;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::{SystemTime, UNIX_EPOCH};

use super::auth_types;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError(pub String);

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ApiError {}

pub trait Has<T> {
    fn get(&self) -> &T;
    fn get_mut(&mut self) -> &mut T;
    fn set(&mut self, value: T);
}

pub trait Push<T> {
    type Result;
    fn push(self, value: T) -> Self::Result;
}

#[derive(Debug, Clone, Default)]
pub struct EmptyContext {
    span_id: XSpanIdString,
    auth_data: Option<auth_types::AuthData>,
    authorization: Option<auth_types::Authorization>,
}

impl Has<XSpanIdString> for EmptyContext {
    fn get(&self) -> &XSpanIdString {
        &self.span_id
    }

    fn get_mut(&mut self) -> &mut XSpanIdString {
        &mut self.span_id
    }

    fn set(&mut self, value: XSpanIdString) {
        self.span_id = value;
    }
}

impl Has<Option<auth_types::AuthData>> for EmptyContext {
    fn get(&self) -> &Option<auth_types::AuthData> {
        &self.auth_data
    }

    fn get_mut(&mut self) -> &mut Option<auth_types::AuthData> {
        &mut self.auth_data
    }

    fn set(&mut self, value: Option<auth_types::AuthData>) {
        self.auth_data = value;
    }
}

impl Has<Option<auth_types::Authorization>> for EmptyContext {
    fn get(&self) -> &Option<auth_types::Authorization> {
        &self.authorization
    }

    fn get_mut(&mut self) -> &mut Option<auth_types::Authorization> {
        &mut self.authorization
    }

    fn set(&mut self, value: Option<auth_types::Authorization>) {
        self.authorization = value;
    }
}

impl Push<XSpanIdString> for EmptyContext {
    type Result = Self;

    fn push(mut self, value: XSpanIdString) -> Self::Result {
        self.span_id = value;
        self
    }
}

impl Push<Option<auth_types::AuthData>> for EmptyContext {
    type Result = Self;

    fn push(mut self, value: Option<auth_types::AuthData>) -> Self::Result {
        self.auth_data = value;
        self
    }
}

impl Push<Option<auth_types::Authorization>> for EmptyContext {
    type Result = Self;

    fn push(mut self, value: Option<auth_types::Authorization>) -> Self::Result {
        self.authorization = value;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct XSpanIdString(pub String);

impl XSpanIdString {
    pub fn get_or_generate<B>(request: &Request<B>) -> Self {
        let span = request
            .headers()
            .get("x-span-id")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_nanos().to_string())
                    .unwrap_or_else(|_| "0".to_string())
            });
        Self(span)
    }
}
