//! Request handlers.

use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;

use crate::analytics::mileage::MonthDay;
use crate::analytics::{self, mileage, timeline::OdometerTimeline, StatsParams, StatsReport};
use crate::i18n::{Lang, Tr};
use crate::importer::{csv::CsvSource, DatasetSource};
use crate::web::session::{PreferredLang, SessionDataset, LANG_COOKIE, SESSION_COOKIE};
use crate::web::views;
use crate::web::AppState;

fn today() -> jiff::civil::Date {
    jiff::Zoned::now().date()
}

#[derive(Deserialize)]
pub struct IndexQuery {
    #[serde(default)]
    expired: Option<u8>,
}

pub async fn index(
    PreferredLang(lang): PreferredLang,
    Query(query): Query<IndexQuery>,
) -> views::IndexPage {
    views::IndexPage {
        t: Tr::new(lang),
        lang_code: lang.code(),
        error: None,
        expired: query.expired.is_some(),
    }
}

pub async fn upload(
    State(state): State<AppState>,
    PreferredLang(lang): PreferredLang,
    jar: CookieJar,
    multipart: Multipart,
) -> Response {
    let t = Tr::new(lang);
    let error_page = |key: &str| views::IndexPage {
        t,
        lang_code: lang.code(),
        error: Some(t.m(key)),
        expired: false,
    };

    let bytes = match read_file_field(multipart, state.config.max_upload_bytes).await {
        Ok(bytes) => bytes,
        Err(key) => {
            return (StatusCode::UNPROCESSABLE_ENTITY, error_page(key)).into_response();
        }
    };

    match CsvSource::new(bytes).fetch().await {
        Ok(dataset) => {
            let id = state.sessions.insert(dataset);
            let cookie = Cookie::build((SESSION_COOKIE, id.to_string()))
                .path("/")
                .http_only(true)
                .same_site(SameSite::Lax)
                .build();
            (jar.add(cookie), Redirect::to("/stats")).into_response()
        }
        Err(e) => {
            tracing::info!(error = %e, "import rejected");
            (StatusCode::UNPROCESSABLE_ENTITY, error_page(e.i18n_key())).into_response()
        }
    }
}

/// Pulls the first uploaded file out of the multipart body.
async fn read_file_field(
    mut multipart: Multipart,
    max_bytes: usize,
) -> Result<Vec<u8>, &'static str> {
    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                if field.file_name().is_none() && field.name() != Some("file") {
                    continue;
                }
                let data = field.bytes().await.map_err(|_| "error.too_large")?;
                if data.len() > max_bytes {
                    return Err("error.too_large");
                }
                if data.is_empty() {
                    return Err("error.no_file");
                }
                return Ok(data.to_vec());
            }
            Ok(None) => return Err("error.no_file"),
            Err(_) => return Err("error.too_large"),
        }
    }
}

pub async fn stats(
    State(_state): State<AppState>,
    PreferredLang(lang): PreferredLang,
    SessionDataset(dataset): SessionDataset,
) -> views::StatsPage {
    let params = StatsParams::new(today());
    let report = StatsReport::compute(&dataset, &params);
    views::StatsPage {
        t: Tr::new(lang),
        lang_code: lang.code(),
        v: views::stats_view(&report, &dataset, lang),
        ins: views::insurance_view(
            report.insurance.as_ref(),
            params.anniversary,
            report.anchor,
            lang,
        ),
        fc: views::forecast_view(
            report.forecast.as_ref(),
            &report.monthly_km,
            params.forecast_months,
            report.anchor,
            lang,
        ),
    }
}

#[derive(Deserialize)]
pub struct InsuranceQuery {
    #[serde(default)]
    anniversary: String,
}

pub async fn partial_insurance(
    State(_state): State<AppState>,
    PreferredLang(lang): PreferredLang,
    SessionDataset(dataset): SessionDataset,
    Query(query): Query<InsuranceQuery>,
) -> views::InsurancePanel {
    let anniversary =
        MonthDay::parse(&query.anniversary).unwrap_or(MonthDay { month: 1, day: 1 });
    let tl = OdometerTimeline::from_dataset(&dataset);
    let anchor = analytics::anchor_date(&tl, today());
    let projection = mileage::insurance_projection(&tl, anniversary, anchor);
    views::InsurancePanel {
        t: Tr::new(lang),
        ins: views::insurance_view(projection.as_ref(), anniversary, anchor, lang),
    }
}

#[derive(Deserialize)]
pub struct ForecastQuery {
    #[serde(default = "default_forecast_months")]
    months: u32,
}

fn default_forecast_months() -> u32 {
    StatsParams::DEFAULT_FORECAST_MONTHS
}

pub async fn partial_forecast(
    State(_state): State<AppState>,
    PreferredLang(lang): PreferredLang,
    SessionDataset(dataset): SessionDataset,
    Query(query): Query<ForecastQuery>,
) -> views::ForecastPanel {
    let months = query.months.clamp(3, 48);
    let tl = OdometerTimeline::from_dataset(&dataset);
    let anchor = analytics::anchor_date(&tl, today());
    let forecast = mileage::forecast(&tl, months, anchor);
    let monthly = mileage::monthly_km(&tl);
    views::ForecastPanel {
        t: Tr::new(lang),
        fc: views::forecast_view(forecast.as_ref(), &monthly, months, anchor, lang),
    }
}

pub async fn switch_lang(
    Path(code): Path<String>,
    headers: HeaderMap,
    jar: CookieJar,
) -> Response {
    let Some(lang) = Lang::from_code(&code) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let cookie = Cookie::build((LANG_COOKIE, lang.code()))
        .path("/")
        .same_site(SameSite::Lax)
        .build();
    let back = headers
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with('/') || v.contains("://"))
        .unwrap_or("/");
    (jar.add(cookie), Redirect::to(back)).into_response()
}
