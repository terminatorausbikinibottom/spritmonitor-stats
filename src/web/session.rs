//! The bridge between "stateless app" and "interactive dashboard".
//!
//! Uploaded datasets live in an in-memory cache keyed by a session cookie, so
//! HTMX tweaks (anniversary date, forecast window) don't re-upload the file.
//! Losing the cache on restart is fine — the user just drops the CSV again.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::CookieJar;
use uuid::Uuid;

use crate::domain::Dataset;
use crate::i18n::Lang;
use crate::web::error::SessionExpired;
use crate::web::AppState;

pub const SESSION_COOKIE: &str = "sid";

#[derive(Clone)]
pub struct SessionStore {
    cache: moka::sync::Cache<Uuid, Arc<Dataset>>,
}

impl SessionStore {
    pub fn new(ttl: Duration, capacity: u64) -> Self {
        Self {
            cache: moka::sync::Cache::builder()
                .time_to_idle(ttl)
                .max_capacity(capacity)
                .build(),
        }
    }

    pub fn insert(&self, dataset: Dataset) -> Uuid {
        let id = Uuid::new_v4();
        self.cache.insert(id, Arc::new(dataset));
        id
    }

    pub fn get(&self, id: &Uuid) -> Option<Arc<Dataset>> {
        self.cache.get(id)
    }
}

/// Extractor for handlers that need the uploaded dataset. Rejects with a
/// localized "session expired" response when there is none.
pub struct SessionDataset(pub Arc<Dataset>);

impl FromRequestParts<AppState> for SessionDataset {
    type Rejection = SessionExpired;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let lang = lang_from_parts(parts, state);
        let is_htmx = parts.headers.contains_key("hx-request");
        let reject = || SessionExpired { lang, is_htmx };

        let jar = CookieJar::from_headers(&parts.headers);
        let id = jar
            .get(SESSION_COOKIE)
            .and_then(|c| Uuid::parse_str(c.value()).ok())
            .ok_or_else(reject)?;
        let dataset = state.sessions.get(&id).ok_or_else(reject)?;
        Ok(Self(dataset))
    }
}

/// Language preference: cookie, then Accept-Language, then the configured
/// default.
pub struct PreferredLang(pub Lang);

impl FromRequestParts<AppState> for PreferredLang {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(lang_from_parts(parts, state)))
    }
}

pub const LANG_COOKIE: &str = "lang";

fn lang_from_parts(parts: &Parts, state: &AppState) -> Lang {
    let jar = CookieJar::from_headers(&parts.headers);
    jar.get(LANG_COOKIE)
        .and_then(|c| Lang::from_code(c.value()))
        .or_else(|| {
            parts
                .headers
                .get("accept-language")
                .and_then(|v| v.to_str().ok())
                .and_then(Lang::from_accept_language)
        })
        .unwrap_or(state.config.default_lang)
}
