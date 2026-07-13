pub mod analytics;
pub mod charts;
pub mod config;
pub mod domain;
pub mod i18n;
pub mod importer;
pub mod web;

rust_i18n::i18n!("locales", fallback = "en");
