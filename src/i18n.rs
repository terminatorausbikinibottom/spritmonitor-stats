//! Language selection and locale-aware formatting.
//!
//! All user-visible strings live in `locales/app.yml`; templates access them
//! through [`Tr`]. Numbers and dates are formatted in Rust so templates stay
//! dumb.

use jiff::civil::Date;
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Self::De => "de",
            Self::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_lowercase().as_str() {
            "de" => Some(Self::De),
            "en" => Some(Self::En),
            _ => None,
        }
    }

    /// Picks the first supported language mentioned in an Accept-Language
    /// header. Not a full RFC 4647 matcher — two languages don't need one.
    pub fn from_accept_language(header: &str) -> Option<Self> {
        header
            .split(',')
            .filter_map(|part| part.split(';').next())
            .find_map(|tag| Self::from_code(tag.trim().get(..2).unwrap_or("")))
    }
}

/// Translator handed to templates: `{{ t.m("stats.title") }}`.
#[derive(Debug, Clone, Copy)]
pub struct Tr {
    pub lang: Lang,
}

impl Tr {
    pub fn new(lang: Lang) -> Self {
        Self { lang }
    }

    pub fn m(&self, key: &str) -> String {
        rust_i18n::t!(key, locale = self.lang.code()).into_owned()
    }
}

/// Formats with the locale's separators: de `12.345,6`, en `12,345.6`.
pub fn fmt_num(lang: Lang, value: f64, decimals: usize) -> String {
    let (dec_sep, thou_sep) = match lang {
        Lang::De => (',', '.'),
        Lang::En => ('.', ','),
    };
    let raw = format!("{value:.decimals$}");
    let (int_part, frac_part) = raw.split_once('.').unwrap_or((raw.as_str(), ""));
    let (sign, digits) = int_part
        .strip_prefix('-')
        .map_or(("", int_part), |d| ("-", d));

    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3 + 4);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(thou_sep);
        }
        grouped.push(c);
    }

    let mut out = format!("{sign}{grouped}");
    if !frac_part.is_empty() {
        out.push(dec_sep);
        out.push_str(frac_part);
    }
    out
}

pub fn fmt_int(lang: Lang, value: f64) -> String {
    fmt_num(lang, value.round(), 0)
}

pub fn fmt_date(lang: Lang, date: Date) -> String {
    match lang {
        Lang::De => format!("{:02}.{:02}.{}", date.day(), date.month(), date.year()),
        Lang::En => format!("{}-{:02}-{:02}", date.year(), date.month(), date.day()),
    }
}

pub fn fmt_money(lang: Lang, value: Decimal, currency: &str) -> String {
    use rust_decimal::prelude::ToPrimitive;
    let amount = fmt_num(lang, value.to_f64().unwrap_or(0.0), 2);
    format!("{amount} {currency}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_formatting() {
        assert_eq!(fmt_num(Lang::De, 12345.6, 1), "12.345,6");
        assert_eq!(fmt_num(Lang::En, 12345.6, 1), "12,345.6");
        assert_eq!(fmt_num(Lang::De, -1234.0, 0), "-1.234");
        assert_eq!(fmt_num(Lang::En, 999.0, 0), "999");
    }

    #[test]
    fn accept_language() {
        assert_eq!(
            Lang::from_accept_language("de-DE,de;q=0.9,en;q=0.8"),
            Some(Lang::De)
        );
        assert_eq!(Lang::from_accept_language("fr-FR,fr;q=0.9"), None);
        assert_eq!(Lang::from_accept_language("en-US"), Some(Lang::En));
    }
}
