use super::credential_cache::CredentialCache;
use super::htpasswd::HtpasswdFile;
use crate::server::transport_types::auth_types::{
    AllowAllAuthenticator, Authorization, Basic, Bearer, RcBound, Scopes, from_headers,
};
use crate::server::transport_types::context_types::ApiError;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

/// Shared htpasswd state that can be reloaded at runtime.
pub type SharedHtpasswd = Arc<RwLock<Option<HtpasswdFile>>>;

/// Shared credential cache that can be cleared on reload.
pub type SharedCredentialCache = Arc<RwLock<Option<CredentialCache>>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub company: String,
    pub exp: u64,
    pub scopes: String,
}

pub trait AuthenticationApi {
    /// Method should be implemented (see example-code) to map Bearer-token to an Authorization
    fn bearer_authorization(&self, token: &Bearer) -> Result<Authorization, ApiError>;

    /// Method should be implemented (see example-code) to map ApiKey to an Authorization
    fn apikey_authorization(&self, token: &str) -> Result<Authorization, ApiError>;

    /// Method should be implemented (see example-code) to map Basic (Username:password) to an Authorization
    fn basic_authorization(&self, basic: &Basic) -> Result<Authorization, ApiError>;
}

/// Custom authenticator that uses htpasswd file for basic authentication
#[derive(Clone)]
pub struct HtpasswdAuthenticator {
    htpasswd: Option<HtpasswdFile>,
    require_auth: bool,
}

impl HtpasswdAuthenticator {
    /// Create a new authenticator with optional htpasswd file
    /// If htpasswd is None and require_auth is false, all requests are allowed (backward compatible)
    /// If require_auth is true, authentication is required
    pub fn new(htpasswd: Option<HtpasswdFile>, require_auth: bool) -> Self {
        HtpasswdAuthenticator {
            htpasswd,
            require_auth,
        }
    }

    fn create_authorization(username: String) -> Authorization {
        Authorization {
            subject: username,
            scopes: Scopes::Some(BTreeSet::new()),
            issuer: None,
        }
    }

    fn unauthorized_error() -> ApiError {
        ApiError("Unauthorized: Invalid username or password".to_string())
    }

    fn auth_required_error() -> ApiError {
        ApiError("Unauthorized: Authentication required".to_string())
    }
}

impl AuthenticationApi for HtpasswdAuthenticator {
    fn bearer_authorization(&self, _token: &Bearer) -> Result<Authorization, ApiError> {
        // Bearer tokens not supported in basic auth mode
        if self.require_auth {
            Err(Self::auth_required_error())
        } else {
            Ok(Self::create_authorization("anonymous".to_string()))
        }
    }

    fn apikey_authorization(&self, _apikey: &str) -> Result<Authorization, ApiError> {
        // API keys not supported in basic auth mode
        if self.require_auth {
            Err(Self::auth_required_error())
        } else {
            Ok(Self::create_authorization("anonymous".to_string()))
        }
    }

    fn basic_authorization(&self, basic: &Basic) -> Result<Authorization, ApiError> {
        match &self.htpasswd {
            Some(htpasswd) => {
                // Basic auth password is always required
                let password = match &basic.password {
                    Some(pwd) => pwd,
                    None => {
                        log::warn!(
                            "Authentication failed for user '{}': no password provided",
                            basic.username
                        );
                        return Err(Self::unauthorized_error());
                    }
                };

                // Verify credentials against htpasswd file
                if htpasswd.verify(&basic.username, password) {
                    log::debug!("User '{}' authenticated successfully", basic.username);
                    Ok(Self::create_authorization(basic.username.clone()))
                } else {
                    log::warn!("Authentication failed for user '{}'", basic.username);
                    Err(Self::unauthorized_error())
                }
            }
            None => {
                // No htpasswd file configured
                if self.require_auth {
                    log::warn!("Authentication required but no htpasswd file configured");
                    Err(Self::auth_required_error())
                } else {
                    // Allow all (backward compatible mode)
                    log::debug!("No authentication configured, allowing request");
                    Ok(Self::create_authorization("anonymous".to_string()))
                }
            }
        }
    }
}

// Implement make service for HtpasswdAuthenticator to work with the local auth/context middleware
use futures::future::FutureExt;
use std::marker::PhantomData;
use std::task::{Context as TaskContext, Poll};
use tower::Service;

/// MakeService wrapper for HtpasswdAuthenticator - creates HtpasswdAuthenticatorService
#[derive(Debug)]
pub struct MakeHtpasswdAuthenticator<T, RC>
where
    RC: RcBound,
    RC::Result: Send + 'static,
{
    inner: T,
    htpasswd: SharedHtpasswd,
    require_auth: bool,
    credential_cache: SharedCredentialCache,
    marker: PhantomData<RC>,
}

impl<T, RC> MakeHtpasswdAuthenticator<T, RC>
where
    RC: RcBound,
    RC::Result: Send + 'static,
{
    pub fn new(
        inner: T,
        htpasswd: SharedHtpasswd,
        require_auth: bool,
        credential_cache: SharedCredentialCache,
    ) -> Self {
        MakeHtpasswdAuthenticator {
            inner,
            htpasswd,
            require_auth,
            credential_cache,
            marker: PhantomData,
        }
    }
}

impl<Inner, RC, Target> Service<Target> for MakeHtpasswdAuthenticator<Inner, RC>
where
    RC: RcBound,
    RC::Result: Send + 'static,
    Inner: Service<Target>,
    Inner::Future: Send + 'static,
{
    type Error = Inner::Error;
    type Response = HtpasswdAuthenticatorService<Inner::Response, RC>;
    type Future = futures::future::BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, target: Target) -> Self::Future {
        let htpasswd = self.htpasswd.clone();
        let require_auth = self.require_auth;
        let credential_cache = self.credential_cache.clone();
        Box::pin(self.inner.call(target).map(move |s| {
            Ok(HtpasswdAuthenticatorService {
                inner: s?,
                htpasswd,
                require_auth,
                credential_cache,
                marker: PhantomData,
            })
        }))
    }
}

/// Service that performs htpasswd authentication on each request
#[derive(Debug)]
pub struct HtpasswdAuthenticatorService<T, RC>
where
    RC: RcBound,
    RC::Result: Send + 'static,
{
    inner: T,
    htpasswd: SharedHtpasswd,
    require_auth: bool,
    credential_cache: SharedCredentialCache,
    marker: PhantomData<RC>,
}

impl<T, RC> HtpasswdAuthenticatorService<T, RC>
where
    RC: RcBound,
    RC::Result: Send + 'static,
{
    /// Verify credentials with caching.
    /// First checks the cache, then falls back to bcrypt verification.
    /// Caches successful verifications.
    fn verify_with_cache(&self, htpasswd: &HtpasswdFile, username: &str, password: &str) -> bool {
        // Check cache first
        let cache_guard = self.credential_cache.read();
        if let Some(ref cache) = *cache_guard
            && cache.is_cached(username, password)
        {
            log::debug!("User '{}' authenticated from cache", username);
            return true;
        }
        drop(cache_guard);

        // Cache miss or no cache - do bcrypt verification
        if htpasswd.verify(username, password) {
            // Cache successful verification
            let cache_guard = self.credential_cache.read();
            if let Some(ref cache) = *cache_guard {
                cache.cache_success(username, password);
            }
            true
        } else {
            false
        }
    }
}

impl<T, RC> Clone for HtpasswdAuthenticatorService<T, RC>
where
    T: Clone,
    RC: RcBound,
    RC::Result: Send + 'static,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            htpasswd: Arc::clone(&self.htpasswd),
            require_auth: self.require_auth,
            credential_cache: Arc::clone(&self.credential_cache),
            marker: PhantomData,
        }
    }
}

impl<T, RC> AuthenticationApi for HtpasswdAuthenticatorService<T, RC>
where
    RC: RcBound,
    RC::Result: Send + 'static,
{
    fn bearer_authorization(&self, _token: &Bearer) -> Result<Authorization, ApiError> {
        // Bearer tokens not supported in basic auth mode
        if self.require_auth {
            Err(ApiError(
                "Unauthorized: Authentication required".to_string(),
            ))
        } else {
            Ok(Authorization {
                subject: "anonymous".to_string(),
                scopes: Scopes::All,
                issuer: None,
            })
        }
    }

    fn apikey_authorization(&self, _apikey: &str) -> Result<Authorization, ApiError> {
        // API keys not supported in basic auth mode
        if self.require_auth {
            Err(ApiError(
                "Unauthorized: Authentication required".to_string(),
            ))
        } else {
            Ok(Authorization {
                subject: "anonymous".to_string(),
                scopes: Scopes::All,
                issuer: None,
            })
        }
    }

    fn basic_authorization(&self, basic: &Basic) -> Result<Authorization, ApiError> {
        let htpasswd_guard = self.htpasswd.read();
        match &*htpasswd_guard {
            Some(htpasswd) => {
                // Basic auth password is always required
                let password = match &basic.password {
                    Some(pwd) => pwd,
                    None => {
                        log::warn!(
                            "Authentication failed for user '{}': no password provided",
                            basic.username
                        );
                        return Err(ApiError(
                            "Unauthorized: Invalid username or password".to_string(),
                        ));
                    }
                };

                // Verify credentials against htpasswd file (with caching)
                if self.verify_with_cache(htpasswd, &basic.username, password) {
                    log::debug!("User '{}' authenticated successfully", basic.username);
                    Ok(Authorization {
                        subject: basic.username.clone(),
                        scopes: Scopes::All,
                        issuer: None,
                    })
                } else {
                    log::warn!("Authentication failed for user '{}'", basic.username);
                    Err(ApiError(
                        "Unauthorized: Invalid username or password".to_string(),
                    ))
                }
            }
            None => {
                // No htpasswd file configured
                if self.require_auth {
                    log::warn!("Authentication required but no htpasswd file configured");
                    Err(ApiError(
                        "Unauthorized: Authentication required".to_string(),
                    ))
                } else {
                    // Allow all (backward compatible mode)
                    log::debug!("No authentication configured, allowing request");
                    Ok(Authorization {
                        subject: "anonymous".to_string(),
                        scopes: Scopes::All,
                        issuer: None,
                    })
                }
            }
        }
    }
}

impl<T, B, RC> Service<(Request<B>, RC)> for HtpasswdAuthenticatorService<T, RC>
where
    RC: RcBound,
    RC::Result: Send + 'static,
    T: Service<(Request<B>, RC::Result)>,
    T::Response: From<Response<Body>>,
{
    type Response = T::Response;
    type Error = T::Error;
    type Future =
        futures::future::Either<futures::future::Ready<Result<T::Response, T::Error>>, T::Future>;

    fn poll_ready(&mut self, cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: (Request<B>, RC)) -> Self::Future {
        let (request, context) = req;

        // Try to extract Basic auth from headers
        let basic_auth: Option<Basic> = from_headers(request.headers());

        let htpasswd_guard = self.htpasswd.read();
        let authorization = match &*htpasswd_guard {
            Some(htpasswd) => {
                // We have an htpasswd file, verify credentials
                match basic_auth {
                    Some(basic) => {
                        // Get password, treating None as empty string
                        let password = basic.password.as_deref().unwrap_or("");

                        if self.verify_with_cache(htpasswd, &basic.username, password) {
                            log::debug!("User '{}' authenticated successfully", basic.username);
                            Some(Authorization {
                                subject: basic.username.clone(),
                                scopes: Scopes::All,
                                issuer: None,
                            })
                        } else {
                            log::warn!("Authentication failed for user '{}'", basic.username);
                            None
                        }
                    }
                    None => {
                        // No credentials provided
                        if self.require_auth {
                            log::warn!("Authentication required but no credentials provided");
                            None
                        } else {
                            // Allow anonymous access (backward compatible)
                            log::debug!("No credentials provided, allowing anonymous access");
                            Some(Authorization {
                                subject: "anonymous".to_string(),
                                scopes: Scopes::All,
                                issuer: None,
                            })
                        }
                    }
                }
            }
            None => {
                // No htpasswd file configured
                if self.require_auth {
                    log::warn!("Authentication required but no htpasswd file configured");
                    None
                } else {
                    // Allow all (backward compatible mode)
                    log::debug!("No authentication configured, allowing request");
                    Some(Authorization {
                        subject: "anonymous".to_string(),
                        scopes: Scopes::All,
                        issuer: None,
                    })
                }
            }
        };
        drop(htpasswd_guard);

        // If require_auth is true and authorization failed, return 401 immediately
        if self.require_auth && authorization.is_none() {
            let response = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("WWW-Authenticate", "Basic realm=\"Torc\"")
                .body(Body::from("Unauthorized"))
                .unwrap();
            return futures::future::Either::Left(futures::future::ready(Ok(response.into())));
        }

        // Push authorization into context and continue
        let context = context.push(authorization);

        futures::future::Either::Right(self.inner.call((request, context)))
    }
}

// Implement it for AllowAllAuthenticator (dummy is needed, but should not used as we have Bearer authorization)

fn dummy_authorization() -> Authorization {
    // Is called when MakeAllowAllAuthenticator is added to the stack. This is not needed as we have Bearer-authorization in the example-code.
    // However, if you want to use it anyway this can not be unimplemented, so dummy implementation added.
    // unimplemented!()
    Authorization {
        subject: "Dummy".to_owned(),
        scopes: Scopes::Some(BTreeSet::new()), // create an empty scope, as this should not be used
        issuer: None,
    }
}

impl<T, RC> AuthenticationApi for AllowAllAuthenticator<T, RC>
where
    RC: RcBound,
    RC::Result: Send + 'static,
{
    /// Get method to map Bearer-token to an Authorization
    fn bearer_authorization(&self, _token: &Bearer) -> Result<Authorization, ApiError> {
        Ok(dummy_authorization())
    }

    /// Get method to map api-key to an Authorization
    fn apikey_authorization(&self, _apikey: &str) -> Result<Authorization, ApiError> {
        Ok(dummy_authorization())
    }

    /// Get method to map basic token to an Authorization
    fn basic_authorization(&self, _basic: &Basic) -> Result<Authorization, ApiError> {
        Ok(dummy_authorization())
    }
}
