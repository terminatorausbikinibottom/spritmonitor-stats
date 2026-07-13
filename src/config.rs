//! Runtime configuration, read once from environment variables at startup.

use std::net::SocketAddr;
use std::time::Duration;

use crate::i18n::Lang;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub max_upload_bytes: usize,
    pub session_ttl: Duration,
    pub session_capacity: u64,
    pub default_lang: Lang,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([0, 0, 0, 0], 8080)),
            max_upload_bytes: 5 * 1024 * 1024,
            session_ttl: Duration::from_secs(7200),
            session_capacity: 100,
            default_lang: Lang::De,
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let defaults = Self::default();
        Ok(Self {
            bind_addr: parse_env("BIND_ADDR", defaults.bind_addr)?,
            max_upload_bytes: parse_env("MAX_UPLOAD_BYTES", defaults.max_upload_bytes)?,
            session_ttl: Duration::from_secs(parse_env(
                "SESSION_TTL_SECS",
                defaults.session_ttl.as_secs(),
            )?),
            session_capacity: parse_env("SESSION_CAPACITY", defaults.session_capacity)?,
            default_lang: match std::env::var("DEFAULT_LANG") {
                Ok(v) => Lang::from_code(&v)
                    .ok_or_else(|| format!("DEFAULT_LANG: unsupported language {v:?}"))?,
                Err(_) => defaults.default_lang,
            },
        })
    }
}

fn parse_env<T: std::str::FromStr>(name: &str, default: T) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    match std::env::var(name) {
        Ok(value) => value.parse().map_err(|e| format!("{name}: {e}")),
        Err(_) => Ok(default),
    }
}
