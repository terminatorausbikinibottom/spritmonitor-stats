//! Statistics computed from a [`Dataset`].
//!
//! Everything here is pure: datasets and parameters in, plain structs out.
//! No HTTP, no rendering, no CSV — which keeps every figure unit-testable.

pub mod behavior;
pub mod consumption;
pub mod costs;
pub mod mileage;
pub mod seasonal;
pub mod timeline;

use jiff::civil::Date;

use crate::domain::Dataset;

#[derive(Debug, Clone, Copy)]
pub struct StatsParams {
    /// "Today" is passed in, never read from a clock, so results are
    /// reproducible in tests.
    pub today: Date,
    pub anniversary: mileage::MonthDay,
    pub forecast_months: u32,
}

impl StatsParams {
    pub const DEFAULT_FORECAST_MONTHS: u32 = 12;

    pub fn new(today: Date) -> Self {
        Self {
            today,
            anniversary: mileage::MonthDay { month: 1, day: 1 },
            forecast_months: Self::DEFAULT_FORECAST_MONTHS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatsReport {
    /// The date every "as of" figure refers to: today, or the last reading
    /// for a dataset that ends in the past.
    pub anchor: Date,
    pub first_date: Option<Date>,
    pub last_date: Option<Date>,
    pub total_km: Option<f64>,
    pub timeline: timeline::OdometerTimeline,
    pub trailing: Option<mileage::TrailingEstimate>,
    pub insurance: Option<mileage::InsuranceProjection>,
    pub calendar_years: Vec<mileage::CalendarYear>,
    pub forecast: Option<mileage::Forecast>,
    pub monthly_km: Vec<mileage::MonthKm>,
    pub consumption: consumption::ConsumptionStats,
    pub costs: costs::CostStats,
    pub seasonal: seasonal::SeasonalStats,
    pub behavior: behavior::BehaviorStats,
}

impl StatsReport {
    pub fn compute(dataset: &Dataset, params: &StatsParams) -> Self {
        let tl = timeline::OdometerTimeline::from_dataset(dataset);
        let anchor = anchor_date(&tl, params.today);

        let consumption = consumption::compute(dataset);
        let seasonal = seasonal::compute(&consumption.points);
        let behavior = behavior::compute(dataset, &consumption.points);

        Self {
            anchor,
            first_date: tl.first().map(|(d, _)| d),
            last_date: tl.last().map(|(d, _)| d),
            total_km: tl
                .first()
                .zip(tl.last())
                .map(|((_, first), (_, last))| last - first),
            trailing: mileage::trailing_12_months(&tl, anchor),
            insurance: mileage::insurance_projection(&tl, params.anniversary, anchor),
            calendar_years: mileage::calendar_years(&tl),
            forecast: mileage::forecast(&tl, params.forecast_months, anchor),
            monthly_km: mileage::monthly_km(&tl),
            consumption,
            costs: costs::compute(dataset),
            seasonal,
            behavior,
            timeline: tl,
        }
    }
}

/// An abandoned export must not silently produce zeros: estimates are
/// anchored at the last reading when the data ends in the past.
pub fn anchor_date(timeline: &timeline::OdometerTimeline, today: Date) -> Date {
    match timeline.last() {
        Some((last, _)) => today.min(last),
        None => today,
    }
}
