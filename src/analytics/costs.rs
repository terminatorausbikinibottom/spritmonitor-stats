//! What the vehicle costs: totals, cost per km, and price trends.

use jiff::civil::Date;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use crate::domain::{Dataset, Fueling};

#[derive(Debug, Clone)]
pub struct CostStats {
    pub total_fuel: Decimal,
    pub total_adblue: Decimal,
    pub total_quantity_liters: f64,
    /// Fuel cost per km over the odometer span covered by priced fuelings.
    pub cost_per_km: Option<f64>,
    /// Fuel spend per calendar year.
    pub per_year: Vec<(i16, Decimal)>,
    /// Fuel spend per calendar month (first of month), for the chart.
    pub per_month: Vec<(Date, Decimal)>,
    pub price_per_liter: Vec<(Date, f64)>,
    pub price_min: Option<(Date, f64)>,
    pub price_max: Option<(Date, f64)>,
    pub price_latest: Option<(Date, f64)>,
}

pub fn compute(dataset: &Dataset) -> CostStats {
    let priced = |f: &&Fueling| f.total_price.is_some();
    let total_fuel: Decimal = dataset
        .fuel_rows()
        .filter_map(|f| f.total_price)
        .sum();
    let total_adblue: Decimal = dataset
        .adblue_rows()
        .filter_map(|f| f.total_price)
        .sum();
    let total_quantity_liters = dataset
        .fuel_rows()
        .filter_map(|f| f.quantity.and_then(|d| d.to_f64()))
        .sum();

    // Cost per km relates the spend to the km actually covered by priced
    // fuelings, so a few price-less rows don't skew the figure.
    let cost_per_km = {
        let with_odo: Vec<&Fueling> = dataset
            .fuel_rows()
            .filter(priced)
            .filter(|f| f.odometer.is_some())
            .collect();
        match (with_odo.first(), with_odo.last()) {
            (Some(first), Some(last)) if first.source_line != last.source_line => {
                let km = (last.odometer.unwrap() - first.odometer.unwrap())
                    .to_f64()
                    .unwrap_or(0.0);
                let spend: Decimal = with_odo.iter().skip(1).filter_map(|f| f.total_price).sum();
                (km > 0.0).then(|| spend.to_f64().unwrap_or(0.0) / km)
            }
            _ => None,
        }
    };

    let mut per_year = std::collections::BTreeMap::<i16, Decimal>::new();
    let mut per_month = std::collections::BTreeMap::<Date, Decimal>::new();
    for f in dataset.fuel_rows().filter(priced) {
        let price = f.total_price.unwrap();
        *per_year.entry(f.date.year()).or_default() += price;
        *per_month.entry(f.date.first_of_month()).or_default() += price;
    }

    let price_per_liter: Vec<(Date, f64)> = dataset
        .fuel_rows()
        .filter_map(|f| {
            let price = f.total_price?.to_f64()?;
            let quantity = f.quantity?.to_f64()?;
            (quantity > 0.0).then(|| (f.date, price / quantity))
        })
        .collect();
    let price_min = price_per_liter
        .iter()
        .copied()
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let price_max = price_per_liter
        .iter()
        .copied()
        .max_by(|a, b| a.1.total_cmp(&b.1));
    let price_latest = price_per_liter.last().copied();

    CostStats {
        total_fuel,
        total_adblue,
        total_quantity_liters,
        cost_per_km,
        per_year: per_year.into_iter().collect(),
        per_month: per_month.into_iter().collect(),
        price_per_liter,
        price_min,
        price_max,
        price_latest,
    }
}
