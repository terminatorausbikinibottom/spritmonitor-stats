//! Fuel consumption, computed correctly across partial fuelings.
//!
//! Consumption is only well-defined at a *full* fueling: everything fueled
//! since the previous full fueling, divided by the odometer distance covered.
//! Partial fuelings roll into the next full one; a first fueling only
//! establishes the baseline.

use jiff::civil::Date;
use rust_decimal::prelude::ToPrimitive;

use crate::domain::{Dataset, DrivingStyle, FuelingKind, Roads, Tires};

/// One closed consumption window, attributed to its closing full fueling.
#[derive(Debug, Clone, Copy)]
pub struct ConsumptionPoint {
    pub date: Date,
    pub liters_per_100km: f64,
    pub distance_km: f64,
    pub quantity_liters: f64,
    // Attributes of the closing fueling, used for seasonal comparisons.
    pub tires: Option<Tires>,
    pub roads: Roads,
    pub style: Option<DrivingStyle>,
}

#[derive(Debug, Clone)]
pub struct ConsumptionStats {
    pub points: Vec<ConsumptionPoint>,
    /// Distance-weighted average over all windows — NOT the mean of the
    /// per-window values, which would over-weight short trips.
    pub overall_l_per_100km: Option<f64>,
    /// Distance-weighted average over the trailing windows, per point.
    pub rolling: Vec<(Date, f64)>,
    pub best: Option<ConsumptionPoint>,
    pub worst: Option<ConsumptionPoint>,
    /// Windows discarded because their value was implausible.
    pub excluded_windows: usize,
}

/// Windows outside this range are treated as data errors, not driving style.
const PLAUSIBLE_L_PER_100KM: std::ops::RangeInclusive<f64> = 1.0..=50.0;
const ROLLING_WINDOWS: usize = 5;

pub fn compute(dataset: &Dataset) -> ConsumptionStats {
    let mut points = Vec::new();
    let mut excluded = 0;

    let mut baseline_km: Option<f64> = None;
    let mut pending_liters = 0.0;
    for f in dataset.fuel_rows() {
        let odometer = f.odometer.and_then(|d| d.to_f64());
        let quantity = f.quantity.and_then(|d| d.to_f64()).unwrap_or(0.0);
        match f.kind {
            FuelingKind::Invalid => {}
            FuelingKind::First => {
                baseline_km = odometer;
                pending_liters = 0.0;
            }
            FuelingKind::Partial => {
                // A partial before any baseline can't be attributed; skip it.
                if baseline_km.is_some() {
                    pending_liters += quantity;
                }
            }
            FuelingKind::Full => {
                // A full fueling without an odometer reading can't close the
                // window; its quantity stays pending for the next one, which
                // keeps the total correct.
                let Some(km) = odometer else {
                    if baseline_km.is_some() {
                        pending_liters += quantity;
                    }
                    continue;
                };
                if let Some(baseline) = baseline_km {
                    let distance = km - baseline;
                    let liters = pending_liters + quantity;
                    if distance > 0.0 && liters > 0.0 {
                        let l_per_100km = liters / distance * 100.0;
                        if PLAUSIBLE_L_PER_100KM.contains(&l_per_100km) {
                            points.push(ConsumptionPoint {
                                date: f.date,
                                liters_per_100km: l_per_100km,
                                distance_km: distance,
                                quantity_liters: liters,
                                tires: f.tires,
                                roads: f.roads,
                                style: f.style,
                            });
                        } else {
                            excluded += 1;
                        }
                    }
                }
                baseline_km = Some(km);
                pending_liters = 0.0;
            }
        }
    }

    let overall = weighted_average(&points);
    let rolling = points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let from = (i + 1).saturating_sub(ROLLING_WINDOWS);
            let avg = weighted_average(&points[from..=i]).expect("window is non-empty");
            (p.date, avg)
        })
        .collect();
    let best = points
        .iter()
        .copied()
        .min_by(|a, b| a.liters_per_100km.total_cmp(&b.liters_per_100km));
    let worst = points
        .iter()
        .copied()
        .max_by(|a, b| a.liters_per_100km.total_cmp(&b.liters_per_100km));

    ConsumptionStats {
        points,
        overall_l_per_100km: overall,
        rolling,
        best,
        worst,
        excluded_windows: excluded,
    }
}

/// Distance-weighted l/100km over a set of windows.
pub fn weighted_average(points: &[ConsumptionPoint]) -> Option<f64> {
    let distance: f64 = points.iter().map(|p| p.distance_km).sum();
    let liters: f64 = points.iter().map(|p| p.quantity_liters).sum();
    (distance > 0.0).then(|| liters / distance * 100.0)
}
