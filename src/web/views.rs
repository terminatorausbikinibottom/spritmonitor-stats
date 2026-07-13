//! Template structs and view models.
//!
//! Analytics produce numbers; this module turns them into localized strings
//! and chart payloads so the templates stay free of logic.

use askama::Template;
use askama_web::WebTemplate;
use jiff::civil::Date;

use crate::analytics::consumption::ConsumptionPoint;
use crate::analytics::mileage::{Forecast, InsuranceProjection, MonthDay, MonthKm};
use crate::analytics::StatsReport;
use crate::charts::Chart;
use crate::domain::{Dataset, ImportWarning, WarningKind};
use crate::i18n::{fmt_date, fmt_int, fmt_money, fmt_num, Lang, Tr};

const DASH: &str = "–";

#[derive(Template, WebTemplate)]
#[template(path = "index.html")]
pub struct IndexPage {
    pub t: Tr,
    pub lang_code: &'static str,
    pub error: Option<String>,
    pub expired: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "stats.html")]
pub struct StatsPage {
    pub t: Tr,
    pub lang_code: &'static str,
    pub v: StatsView,
    pub ins: InsuranceView,
    pub fc: ForecastView,
}

#[derive(Template, WebTemplate)]
#[template(path = "partials/insurance.html")]
pub struct InsurancePanel {
    pub t: Tr,
    pub ins: InsuranceView,
}

#[derive(Template, WebTemplate)]
#[template(path = "partials/forecast.html")]
pub struct ForecastPanel {
    pub t: Tr,
    pub fc: ForecastView,
}

pub struct StatsView {
    pub period_label: String,
    pub anchor_label: String,
    pub total_km: String,
    // Overview cards
    pub trailing_km: String,
    pub trailing_extrapolated: bool,
    pub overall_consumption: String,
    pub cost_per_km: String,
    pub total_spend: String,
    // Calendar years
    pub calendar_rows: Vec<CalendarRow>,
    pub calendar_chart: String,
    // Consumption
    pub has_consumption: bool,
    pub rolling_last: String,
    pub best_label: String,
    pub worst_label: String,
    pub excluded_windows: usize,
    pub consumption_chart: String,
    // Costs
    pub total_adblue: String,
    pub total_quantity: String,
    pub per_year_rows: Vec<(String, String)>,
    pub price_min: String,
    pub price_max: String,
    pub price_latest: String,
    pub price_chart: String,
    // Seasonal comparisons
    pub tires_rows: Vec<GroupRow>,
    pub season_rows: Vec<GroupRow>,
    pub style_rows: Vec<GroupRow>,
    pub roads_rows: Vec<GroupRow>,
    // Behavior
    pub fueling_count: String,
    pub avg_km_between: String,
    pub median_km_between: String,
    pub avg_days_between: String,
    pub avg_fill: String,
    pub tank_range: String,
    pub top_companies: Vec<StationRow>,
    pub top_locations: Vec<StationRow>,
    // Import warnings
    pub warnings: Vec<String>,
}

pub struct CalendarRow {
    pub year: String,
    pub km: String,
    pub annualized: String,
    pub full_year: bool,
}

pub struct GroupRow {
    pub label: String,
    pub consumption: String,
    pub delta: String,
    pub share: String,
}

pub struct StationRow {
    pub name: String,
    pub visits: String,
    pub spend: String,
}

pub struct InsuranceView {
    pub available: bool,
    /// Value for the HTML date input; the year part is only decoration.
    pub input_value: String,
    pub period_label: String,
    pub km_so_far: String,
    pub elapsed_label: String,
    pub pace_projection: String,
    pub recent_projection: String,
    pub prorated: bool,
    pub history: Vec<InsuranceRow>,
}

pub struct InsuranceRow {
    pub label: String,
    pub km: String,
}

pub struct ForecastView {
    pub available: bool,
    pub months: u32,
    pub km: String,
    pub confidence: String,
    pub chart: String,
}

pub fn stats_view(report: &StatsReport, dataset: &Dataset, lang: Lang) -> StatsView {
    let t = Tr::new(lang);
    let currency = dataset.currency.as_deref().unwrap_or("EUR");
    let km = |v: f64| format!("{} km", fmt_int(lang, v));
    let l100 = |v: f64| format!("{} l/100km", fmt_num(lang, v, 1));

    let period_label = match (report.first_date, report.last_date) {
        (Some(a), Some(b)) => format!("{} – {}", fmt_date(lang, a), fmt_date(lang, b)),
        _ => DASH.to_string(),
    };

    let calendar_rows: Vec<CalendarRow> = report
        .calendar_years
        .iter()
        .map(|y| CalendarRow {
            year: y.year.to_string(),
            km: km(y.km),
            annualized: km(y.annualized),
            full_year: y.full_year,
        })
        .collect();
    let calendar_chart = Chart::ordinal_bars(
        lang,
        "km",
        report.calendar_years.iter().map(|y| y.year.to_string()).collect(),
    )
    .with_series(
        t.m("mileage.calendar_series"),
        report.calendar_years.iter().map(|y| Some(y.km.round())).collect(),
    )
    .to_json();

    let consumption = &report.consumption;
    let point_label = |p: &ConsumptionPoint| {
        format!("{} ({})", l100(p.liters_per_100km), fmt_date(lang, p.date))
    };

    let costs = &report.costs;
    let money = |v| fmt_money(lang, v, currency);
    let price = |p: Option<(Date, f64)>| match p {
        Some((d, v)) => format!(
            "{} {}/l ({})",
            fmt_num(lang, v, 3),
            currency,
            fmt_date(lang, d)
        ),
        None => DASH.to_string(),
    };

    let behavior = &report.behavior;
    let group_rows = |groups: &[crate::analytics::seasonal::GroupStat]| -> Vec<GroupRow> {
        groups
            .iter()
            .map(|g| GroupRow {
                label: t.m(&format!("seasonal.{}", g.key)),
                consumption: l100(g.liters_per_100km),
                delta: match consumption.overall_l_per_100km {
                    Some(overall) => {
                        let delta = g.liters_per_100km - overall;
                        format!("{}{}", if delta >= 0.0 { "+" } else { "" }, fmt_num(lang, delta, 1))
                    }
                    None => DASH.to_string(),
                },
                share: format!("{} %", fmt_int(lang, g.share * 100.0)),
            })
            .collect()
    };
    let station_rows = |stations: &[crate::analytics::behavior::StationStat]| {
        stations
            .iter()
            .map(|s| StationRow {
                name: s.name.clone(),
                visits: s.visits.to_string(),
                spend: money(s.spend),
            })
            .collect()
    };

    StatsView {
        period_label,
        anchor_label: fmt_date(lang, report.anchor),
        total_km: report.total_km.map(km).unwrap_or_else(|| DASH.into()),
        trailing_km: report
            .trailing
            .map(|tr| km(tr.km))
            .unwrap_or_else(|| DASH.into()),
        trailing_extrapolated: report.trailing.is_some_and(|tr| tr.extrapolated),
        overall_consumption: consumption
            .overall_l_per_100km
            .map(l100)
            .unwrap_or_else(|| DASH.into()),
        cost_per_km: costs
            .cost_per_km
            .map(|v| format!("{} {}/km", fmt_num(lang, v, 2), currency))
            .unwrap_or_else(|| DASH.into()),
        total_spend: money(costs.total_fuel),
        calendar_rows,
        calendar_chart,
        has_consumption: !consumption.points.is_empty(),
        rolling_last: consumption
            .rolling
            .last()
            .map(|(_, v)| l100(*v))
            .unwrap_or_else(|| DASH.into()),
        best_label: consumption
            .best
            .as_ref()
            .map(point_label)
            .unwrap_or_else(|| DASH.into()),
        worst_label: consumption
            .worst
            .as_ref()
            .map(point_label)
            .unwrap_or_else(|| DASH.into()),
        excluded_windows: consumption.excluded_windows,
        consumption_chart: consumption_chart(consumption, lang, &t),
        total_adblue: (costs.total_adblue > rust_decimal::Decimal::ZERO)
            .then(|| money(costs.total_adblue))
            .unwrap_or_default(),
        total_quantity: format!("{} l", fmt_int(lang, costs.total_quantity_liters)),
        per_year_rows: costs
            .per_year
            .iter()
            .map(|(year, spend)| (year.to_string(), money(*spend)))
            .collect(),
        price_min: price(costs.price_min),
        price_max: price(costs.price_max),
        price_latest: price(costs.price_latest),
        price_chart: price_chart(costs, currency, lang, &t),
        tires_rows: group_rows(&report.seasonal.by_tires),
        season_rows: group_rows(&report.seasonal.by_season),
        style_rows: group_rows(&report.seasonal.by_style),
        roads_rows: group_rows(&report.seasonal.by_roads),
        fueling_count: behavior.fueling_count.to_string(),
        avg_km_between: behavior
            .avg_km_between_fuelings
            .map(km)
            .unwrap_or_else(|| DASH.into()),
        median_km_between: behavior
            .median_km_between_fuelings
            .map(km)
            .unwrap_or_else(|| DASH.into()),
        avg_days_between: behavior
            .avg_days_between_fuelings
            .map(|v| fmt_num(lang, v, 1))
            .unwrap_or_else(|| DASH.into()),
        avg_fill: behavior
            .avg_fill_liters
            .map(|v| format!("{} l", fmt_num(lang, v, 1)))
            .unwrap_or_else(|| DASH.into()),
        tank_range: behavior
            .tank_range_km
            .map(km)
            .unwrap_or_else(|| DASH.into()),
        top_companies: station_rows(&behavior.top_companies),
        top_locations: station_rows(&behavior.top_locations),
        warnings: dataset
            .warnings
            .iter()
            .map(|w| warning_text(w, &t))
            .collect(),
    }
}

pub fn insurance_view(
    projection: Option<&InsuranceProjection>,
    anniversary: MonthDay,
    anchor: Date,
    lang: Lang,
) -> InsuranceView {
    let t = Tr::new(lang);
    let km = |v: f64| format!("{} km", fmt_int(lang, v));
    let input_value = format!(
        "{:04}-{:02}-{:02}",
        anchor.year(),
        anniversary.month,
        anniversary.day
    );

    let Some(p) = projection else {
        return InsuranceView {
            available: false,
            input_value,
            period_label: String::new(),
            km_so_far: String::new(),
            elapsed_label: String::new(),
            pace_projection: String::new(),
            recent_projection: String::new(),
            prorated: false,
            history: Vec::new(),
        };
    };

    InsuranceView {
        available: true,
        input_value,
        period_label: format!(
            "{} – {}",
            fmt_date(lang, p.period_start),
            fmt_date(lang, p.period_end)
        ),
        km_so_far: km(p.km_so_far),
        elapsed_label: t
            .m("insurance.elapsed")
            .replace("%{elapsed}", &p.elapsed_days.to_string())
            .replace("%{total}", &p.year_length_days.to_string()),
        pace_projection: km(p.pace_projection),
        recent_projection: p
            .recent_projection
            .map(km)
            .unwrap_or_default(),
        prorated: p.prorated,
        history: p
            .history
            .iter()
            .map(|y| InsuranceRow {
                label: format!(
                    "{} – {}",
                    fmt_date(lang, y.start),
                    fmt_date(lang, y.end)
                ),
                km: km(y.km),
            })
            .collect(),
    }
}

pub fn forecast_view(
    forecast: Option<&Forecast>,
    monthly: &[MonthKm],
    months: u32,
    anchor: Date,
    lang: Lang,
) -> ForecastView {
    let t = Tr::new(lang);
    let Some(f) = forecast else {
        return ForecastView {
            available: false,
            months,
            km: String::new(),
            confidence: String::new(),
            chart: String::new(),
        };
    };

    let confidence_key = match f.r_squared {
        r if r >= 0.98 => "forecast.confidence_high",
        r if r >= 0.90 => "forecast.confidence_medium",
        _ => "forecast.confidence_low",
    };
    ForecastView {
        available: true,
        months,
        km: format!("{} km", fmt_int(lang, f.next_12_months_km)),
        confidence: t
            .m(confidence_key)
            .replace("%{r2}", &fmt_num(lang, f.r_squared, 2)),
        chart: forecast_chart(f, monthly, anchor, lang, &t),
    }
}

fn consumption_chart(
    consumption: &crate::analytics::consumption::ConsumptionStats,
    lang: Lang,
    t: &Tr,
) -> String {
    let epoch = crate::analytics::timeline::epoch_seconds;
    Chart::time(lang, "l/100km")
        .with_series(
            t.m("consumption.series"),
            consumption
                .points
                .iter()
                .map(|p| Some((p.liters_per_100km * 100.0).round() / 100.0))
                .collect(),
        )
        .with_dashed_series(
            t.m("consumption.rolling_series"),
            consumption
                .rolling
                .iter()
                .map(|(_, v)| Some((v * 100.0).round() / 100.0))
                .collect(),
        )
        .with_x(consumption.points.iter().map(|p| epoch(p.date) as f64).collect())
        .to_json()
}

fn price_chart(
    costs: &crate::analytics::costs::CostStats,
    currency: &str,
    lang: Lang,
    t: &Tr,
) -> String {
    let epoch = crate::analytics::timeline::epoch_seconds;
    Chart::time(lang, format!("{currency}/l"))
        .with_series(
            t.m("costs.price_series"),
            costs
                .price_per_liter
                .iter()
                .map(|(_, v)| Some((v * 1000.0).round() / 1000.0))
                .collect(),
        )
        .with_x(
            costs
                .price_per_liter
                .iter()
                .map(|(d, _)| epoch(*d) as f64)
                .collect(),
        )
        .to_json()
}

/// Monthly km bars plus the regression pace, extended a year into the future.
fn forecast_chart(
    forecast: &Forecast,
    monthly: &[MonthKm],
    anchor: Date,
    lang: Lang,
    t: &Tr,
) -> String {
    use jiff::ToSpan;
    let epoch = crate::analytics::timeline::epoch_seconds;
    const AVG_DAYS_PER_MONTH: f64 = 30.44;

    let mut x = Vec::new();
    let mut actual = Vec::new();
    let mut pace = Vec::new();
    for m in monthly {
        x.push(epoch(m.month) as f64);
        actual.push(Some(m.km.round()));
        pace.push(
            (m.month >= forecast.window_start.first_of_month())
                .then(|| (forecast.slope_km_per_day * AVG_DAYS_PER_MONTH).round()),
        );
    }
    let mut future = anchor.first_of_month();
    for _ in 0..12 {
        future = future.saturating_add(1.month()).first_of_month();
        x.push(epoch(future) as f64);
        actual.push(None);
        pace.push(Some((forecast.slope_km_per_day * AVG_DAYS_PER_MONTH).round()));
    }

    Chart::time(lang, "km")
        .with_series(t.m("forecast.actual_series"), actual)
        .with_dashed_series(t.m("forecast.pace_series"), pace)
        .with_x(x)
        .to_json()
}

fn warning_text(warning: &ImportWarning, t: &Tr) -> String {
    let text = match &warning.kind {
        WarningKind::Windows1252Fallback => t.m("warning.encoding"),
        WarningKind::PreambleSkipped => t.m("warning.preamble"),
        WarningKind::UnknownColumn(name) => t.m("warning.unknown_column").replace("%{name}", name),
        WarningKind::RowSkipped { value } => t.m("warning.row_skipped").replace("%{value}", value),
        WarningKind::BadValue { column, value } => t
            .m("warning.bad_value")
            .replace("%{column}", column)
            .replace("%{value}", value),
        WarningKind::OdometerReconstructed => t.m("warning.odometer_reconstructed"),
        WarningKind::OdometerNonMonotonic => t.m("warning.odometer_backwards"),
    };
    match warning.line {
        Some(line) => format!("{} {line}: {text}", t.m("warning.line")),
        None => text,
    }
}
