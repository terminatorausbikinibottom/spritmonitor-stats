//! Fueling habits: how often, how much, how far, and where.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use crate::analytics::consumption::ConsumptionPoint;
use crate::analytics::timeline::days_between;
use crate::domain::Dataset;

#[derive(Debug, Clone)]
pub struct BehaviorStats {
    pub fueling_count: usize,
    pub avg_km_between_fuelings: Option<f64>,
    pub median_km_between_fuelings: Option<f64>,
    pub avg_days_between_fuelings: Option<f64>,
    pub avg_fill_liters: Option<f64>,
    /// Histogram of fill-up sizes in 5-liter buckets: (bucket start, count).
    pub fill_histogram: Vec<(u32, usize)>,
    /// 95th percentile of full-to-full distances — a robust tank range.
    pub tank_range_km: Option<f64>,
    pub top_companies: Vec<StationStat>,
    pub top_locations: Vec<StationStat>,
}

#[derive(Debug, Clone)]
pub struct StationStat {
    pub name: String,
    pub visits: usize,
    pub spend: Decimal,
}

const HISTOGRAM_BUCKET_LITERS: u32 = 5;
const TOP_N: usize = 5;

pub fn compute(dataset: &Dataset, consumption_points: &[ConsumptionPoint]) -> BehaviorStats {
    let rows: Vec<_> = dataset.fuel_rows().collect();

    let mut km_gaps: Vec<f64> = rows
        .windows(2)
        .filter_map(|w| {
            let a = w[0].odometer?.to_f64()?;
            let b = w[1].odometer?.to_f64()?;
            (b > a).then_some(b - a)
        })
        .collect();
    let avg_km = mean(&km_gaps);
    km_gaps.sort_by(f64::total_cmp);
    let median_km = percentile(&km_gaps, 0.5);

    let day_gaps: Vec<f64> = rows
        .windows(2)
        .map(|w| days_between(w[0].date, w[1].date) as f64)
        .collect();
    let avg_days = mean(&day_gaps);

    let fills: Vec<f64> = rows
        .iter()
        .filter(|f| f.kind == crate::domain::FuelingKind::Full)
        .filter_map(|f| f.quantity?.to_f64())
        .filter(|&q| q > 0.0)
        .collect();
    let avg_fill = mean(&fills);
    let fill_histogram = histogram(&fills);

    let mut window_distances: Vec<f64> =
        consumption_points.iter().map(|p| p.distance_km).collect();
    window_distances.sort_by(f64::total_cmp);
    let tank_range_km = percentile(&window_distances, 0.95);

    BehaviorStats {
        fueling_count: rows.len(),
        avg_km_between_fuelings: avg_km,
        median_km_between_fuelings: median_km,
        avg_days_between_fuelings: avg_days,
        avg_fill_liters: avg_fill,
        fill_histogram,
        tank_range_km,
        top_companies: top_stations(&rows, |f| f.company.as_deref()),
        top_locations: top_stations(&rows, |f| f.location.as_deref()),
    }
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

/// Nearest-rank percentile of an already sorted slice.
fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = ((sorted.len() as f64 * p).ceil() as usize).clamp(1, sorted.len());
    Some(sorted[rank - 1])
}

fn histogram(fills: &[f64]) -> Vec<(u32, usize)> {
    let mut buckets = std::collections::BTreeMap::<u32, usize>::new();
    for &fill in fills {
        let bucket = (fill as u32 / HISTOGRAM_BUCKET_LITERS) * HISTOGRAM_BUCKET_LITERS;
        *buckets.entry(bucket).or_default() += 1;
    }
    buckets.into_iter().collect()
}

fn top_stations<'a>(
    rows: &[&'a crate::domain::Fueling],
    name_of: impl Fn(&'a crate::domain::Fueling) -> Option<&'a str>,
) -> Vec<StationStat> {
    let mut by_name = std::collections::HashMap::<&str, (usize, Decimal)>::new();
    for f in rows {
        if let Some(name) = name_of(f).filter(|n| !n.is_empty()) {
            let entry = by_name.entry(name).or_default();
            entry.0 += 1;
            entry.1 += f.total_price.unwrap_or_default();
        }
    }
    let mut stats: Vec<StationStat> = by_name
        .into_iter()
        .map(|(name, (visits, spend))| StationStat {
            name: name.to_string(),
            visits,
            spend,
        })
        .collect();
    stats.sort_by(|a, b| b.visits.cmp(&a.visits).then(b.spend.cmp(&a.spend)));
    stats.truncate(TOP_N);
    stats
}
