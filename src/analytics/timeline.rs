//! The odometer timeline: the foundation every mileage estimate builds on.

use jiff::civil::Date;
use rust_decimal::prelude::ToPrimitive;

use crate::domain::Dataset;

/// Whole days from `a` to `b` (negative if `b` is earlier).
pub fn days_between(a: Date, b: Date) -> i64 {
    b.since(a).expect("civil dates are always comparable").get_days() as i64
}

/// Unix timestamp of midnight UTC, used as the x value in time charts.
pub fn epoch_seconds(date: Date) -> i64 {
    date.to_zoned(jiff::tz::TimeZone::UTC)
        .expect("UTC has no gaps")
        .timestamp()
        .as_second()
}

/// Odometer readings over time: strictly increasing dates, non-decreasing km.
#[derive(Debug, Clone)]
pub struct OdometerTimeline {
    points: Vec<(Date, f64)>,
}

impl OdometerTimeline {
    pub fn from_dataset(dataset: &Dataset) -> Self {
        // One point per day: the highest reading wins on multi-fueling days.
        let mut by_day = std::collections::BTreeMap::<Date, f64>::new();
        for f in dataset.fuel_rows() {
            if let Some(km) = f.odometer.and_then(|d| d.to_f64()) {
                let entry = by_day.entry(f.date).or_insert(km);
                *entry = entry.max(km);
            }
        }

        // The dataset already discards backwards readings; this guards the
        // invariant against any future source that doesn't.
        let mut points = Vec::with_capacity(by_day.len());
        let mut max_km = f64::MIN;
        for (date, km) in by_day {
            if km >= max_km {
                points.push((date, km));
                max_km = km;
            }
        }
        Self { points }
    }

    pub fn points(&self) -> &[(Date, f64)] {
        &self.points
    }

    pub fn first(&self) -> Option<(Date, f64)> {
        self.points.first().copied()
    }

    pub fn last(&self) -> Option<(Date, f64)> {
        self.points.last().copied()
    }

    pub fn span_days(&self) -> i64 {
        match (self.first(), self.last()) {
            (Some((a, _)), Some((b, _))) => days_between(a, b),
            _ => 0,
        }
    }

    /// Estimated odometer reading on `date`, linearly interpolated between
    /// the surrounding readings. `None` outside the recorded range.
    pub fn km_at(&self, date: Date) -> Option<f64> {
        let (first, last) = (self.first()?, self.last()?);
        if date < first.0 || date > last.0 {
            return None;
        }
        match self.points.binary_search_by(|p| p.0.cmp(&date)) {
            Ok(i) => Some(self.points[i].1),
            Err(i) => {
                let (d0, k0) = self.points[i - 1];
                let (d1, k1) = self.points[i];
                let whole = days_between(d0, d1) as f64;
                let part = days_between(d0, date) as f64;
                Some(k0 + (k1 - k0) * part / whole)
            }
        }
    }

    /// Km covered between two dates, both clamped to the recorded range.
    pub fn km_between(&self, from: Date, to: Date) -> Option<f64> {
        let (first, last) = (self.first()?, self.last()?);
        let from = from.max(first.0);
        let to = to.min(last.0);
        if from > to {
            return None;
        }
        Some(self.km_at(to)? - self.km_at(from)?)
    }
}
