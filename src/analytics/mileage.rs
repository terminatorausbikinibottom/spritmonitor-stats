//! Yearly mileage estimation — the numbers the insurance company asks for.
//!
//! Everything is derived from interpolated odometer readings
//! ([`OdometerTimeline::km_at`]), so the math is robust against missing trip
//! distances and irregular fueling intervals.

use jiff::civil::Date;
use jiff::ToSpan;

use crate::analytics::timeline::{days_between, OdometerTimeline};

const DAYS_PER_YEAR: f64 = 365.25;

/// A recurring day of the year, e.g. the insurance policy anniversary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthDay {
    pub month: i8,
    pub day: i8,
}

impl MonthDay {
    /// Accepts `MM-DD` as well as a full `YYYY-MM-DD` (the year is ignored),
    /// so a plain HTML date input can feed it directly.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let tail = match s.len() {
            5 => s,
            10 => s.get(5..)?,
            _ => return None,
        };
        let (m, d) = tail.split_once('-')?;
        let month = m.parse().ok().filter(|m| (1..=12).contains(m))?;
        let day = d.parse().ok().filter(|d| (1..=31).contains(d))?;
        // Reject days invalid in every year (e.g. 04-31); Feb 29 is fine.
        Date::new(2024, month, day).ok()?;
        Some(Self { month, day })
    }

    /// The occurrence in `year`; Feb 29 falls back to Feb 28 outside leap years.
    pub fn date_in(self, year: i16) -> Date {
        Date::new(year, self.month, self.day)
            .unwrap_or_else(|_| Date::new(year, self.month, self.day - 1).expect("Feb 28 exists"))
    }

    /// The most recent occurrence on or before `anchor`.
    pub fn latest_on_or_before(self, anchor: Date) -> Date {
        let candidate = self.date_in(anchor.year());
        if candidate > anchor {
            self.date_in(anchor.year() - 1)
        } else {
            candidate
        }
    }
}

/// Km driven in the last 365 days.
#[derive(Debug, Clone, Copy)]
pub struct TrailingEstimate {
    pub km: f64,
    /// True when the data covers less than a year and the value is the
    /// annualized pace of the available span instead.
    pub extrapolated: bool,
}

pub fn trailing_12_months(timeline: &OdometerTimeline, anchor: Date) -> Option<TrailingEstimate> {
    let (first_date, first_km) = timeline.first()?;
    let (last_date, last_km) = timeline.last()?;
    let start = anchor.saturating_sub(1.year());

    if start >= first_date {
        let km = timeline.km_at(anchor)? - timeline.km_at(start)?;
        return Some(TrailingEstimate {
            km,
            extrapolated: false,
        });
    }
    let span = days_between(first_date, last_date);
    (span >= 1).then(|| TrailingEstimate {
        km: (last_km - first_km) / span as f64 * DAYS_PER_YEAR,
        extrapolated: true,
    })
}

/// One completed insurance year in the past.
#[derive(Debug, Clone, Copy)]
pub struct InsuranceYear {
    pub start: Date,
    pub end: Date,
    pub km: f64,
}

/// Projection for the currently running insurance year.
#[derive(Debug, Clone)]
pub struct InsuranceProjection {
    pub period_start: Date,
    pub period_end: Date,
    pub km_so_far: f64,
    pub elapsed_days: i64,
    pub year_length_days: i64,
    /// `km_so_far` extended at the average pace of the year so far.
    pub pace_projection: f64,
    /// `km_so_far` plus the remaining days at the trailing-90-day pace —
    /// reacts faster when driving habits changed recently.
    pub recent_projection: Option<f64>,
    /// True when the period start predates the data and the pace is measured
    /// from the first reading instead of the anniversary.
    pub prorated: bool,
    /// Completed past insurance years, most recent first.
    pub history: Vec<InsuranceYear>,
}

pub fn insurance_projection(
    timeline: &OdometerTimeline,
    anniversary: MonthDay,
    anchor: Date,
) -> Option<InsuranceProjection> {
    let (first_date, _) = timeline.first()?;
    let period_start = anniversary.latest_on_or_before(anchor);
    let period_end = anniversary.date_in(period_start.year() + 1);
    let year_length_days = days_between(period_start, period_end);

    let prorated = period_start < first_date;
    let measured_from = period_start.max(first_date);
    let elapsed_days = days_between(measured_from, anchor);
    if elapsed_days < 1 {
        return None;
    }
    let km_so_far = timeline.km_between(measured_from, anchor)?;
    let pace_projection = km_so_far * year_length_days as f64 / elapsed_days as f64;

    let recent_projection = trailing_rate(timeline, anchor, 90).map(|rate| {
        let remaining = (year_length_days - elapsed_days).max(0);
        km_so_far + rate * remaining as f64
    });

    let mut history = Vec::new();
    for year in first_date.year()..=anchor.year() {
        let start = anniversary.date_in(year);
        let end = anniversary.date_in(year + 1);
        if start >= first_date && end <= anchor {
            if let Some(km) = timeline.km_between(start, end) {
                history.push(InsuranceYear { start, end, km });
            }
        }
    }
    history.reverse();

    Some(InsuranceProjection {
        period_start,
        period_end,
        km_so_far,
        elapsed_days,
        year_length_days,
        pace_projection,
        recent_projection,
        prorated,
        history,
    })
}

/// Average km/day over the last `days` days before `anchor`.
fn trailing_rate(timeline: &OdometerTimeline, anchor: Date, days: i64) -> Option<f64> {
    let (first_date, _) = timeline.first()?;
    let start = anchor.saturating_sub((days as i32).days()).max(first_date);
    let elapsed = days_between(start, anchor);
    if elapsed < 1 {
        return None;
    }
    Some(timeline.km_between(start, anchor)? / elapsed as f64)
}

/// Km driven in one calendar year (possibly only partially covered by data).
#[derive(Debug, Clone, Copy)]
pub struct CalendarYear {
    pub year: i16,
    pub km: f64,
    pub covered_days: i64,
    pub full_year: bool,
    /// `km` scaled up to a full year; equal to `km` for fully covered years.
    pub annualized: f64,
}

pub fn calendar_years(timeline: &OdometerTimeline) -> Vec<CalendarYear> {
    let Some(((first_date, _), (last_date, _))) = timeline.first().zip(timeline.last()) else {
        return Vec::new();
    };
    let mut years = Vec::new();
    for year in first_date.year()..=last_date.year() {
        let jan1 = Date::new(year, 1, 1).expect("Jan 1 exists");
        let next_jan1 = Date::new(year + 1, 1, 1).expect("Jan 1 exists");
        let start = jan1.max(first_date);
        let end = next_jan1.min(last_date);
        let covered_days = days_between(start, end);
        if covered_days < 1 {
            continue;
        }
        let Some(km) = timeline.km_between(start, end) else {
            continue;
        };
        let full_year = start == jan1 && end == next_jan1;
        let annualized = if full_year {
            km
        } else {
            km * days_between(jan1, next_jan1) as f64 / covered_days as f64
        };
        years.push(CalendarYear {
            year,
            km,
            covered_days,
            full_year,
            annualized,
        });
    }
    years
}

/// Linear-regression forecast of the next 12 months.
#[derive(Debug, Clone, Copy)]
pub struct Forecast {
    pub window_months: u32,
    pub window_start: Date,
    pub slope_km_per_day: f64,
    pub next_12_months_km: f64,
    /// Coefficient of determination; 1.0 means perfectly steady driving.
    pub r_squared: f64,
    pub points_used: usize,
}

pub fn forecast(timeline: &OdometerTimeline, window_months: u32, anchor: Date) -> Option<Forecast> {
    let window_start = anchor.saturating_sub((window_months as i32).months());
    let points: Vec<(f64, f64)> = timeline
        .points()
        .iter()
        .filter(|(d, _)| *d >= window_start && *d <= anchor)
        .map(|(d, km)| (days_between(window_start, *d) as f64, *km))
        .collect();

    let (first_x, last_x) = (points.first()?.0, points.last()?.0);
    if points.len() < 3 || last_x - first_x < 60.0 {
        return None;
    }
    let (slope, r_squared) = ols(&points)?;
    Some(Forecast {
        window_months,
        window_start,
        slope_km_per_day: slope,
        next_12_months_km: slope * DAYS_PER_YEAR,
        r_squared,
        points_used: points.len(),
    })
}

/// Ordinary least squares; returns (slope, r²).
fn ols(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    let n = points.len() as f64;
    let mean_x = points.iter().map(|(x, _)| x).sum::<f64>() / n;
    let mean_y = points.iter().map(|(_, y)| y).sum::<f64>() / n;

    let mut ss_xy = 0.0;
    let mut ss_xx = 0.0;
    let mut ss_yy = 0.0;
    for (x, y) in points {
        ss_xy += (x - mean_x) * (y - mean_y);
        ss_xx += (x - mean_x) * (x - mean_x);
        ss_yy += (y - mean_y) * (y - mean_y);
    }
    if ss_xx == 0.0 {
        return None;
    }
    let slope = ss_xy / ss_xx;
    // All readings equal => flat line fits perfectly.
    let r_squared = if ss_yy == 0.0 {
        1.0
    } else {
        (ss_xy * ss_xy) / (ss_xx * ss_yy)
    };
    Some((slope, r_squared))
}

/// Km per calendar month, for charts.
#[derive(Debug, Clone, Copy)]
pub struct MonthKm {
    /// First day of the month.
    pub month: Date,
    pub km: f64,
}

pub fn monthly_km(timeline: &OdometerTimeline) -> Vec<MonthKm> {
    let Some(((first_date, _), (last_date, _))) = timeline.first().zip(timeline.last()) else {
        return Vec::new();
    };
    let mut months = Vec::new();
    let mut month_start = first_date.first_of_month();
    while month_start <= last_date {
        let next = month_start.saturating_add(1.month()).first_of_month();
        let from = month_start.max(first_date);
        let to = next.min(last_date);
        if let Some(km) = timeline.km_between(from, to) {
            months.push(MonthKm {
                month: month_start,
                km,
            });
        }
        month_start = next;
    }
    months
}
