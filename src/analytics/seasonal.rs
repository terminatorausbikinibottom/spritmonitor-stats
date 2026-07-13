//! Consumption compared across tires, seasons, road types and driving style.
//!
//! Groups are built from consumption windows, each attributed to the
//! attributes of its closing full fueling — an approximation, but the best
//! the data offers.

use crate::analytics::consumption::{weighted_average, ConsumptionPoint};
use crate::domain::{DrivingStyle, Tires};

/// One row of a comparison table.
#[derive(Debug, Clone)]
pub struct GroupStat {
    /// Suffix of the i18n key `seasonal.<key>`.
    pub key: &'static str,
    pub liters_per_100km: f64,
    pub distance_km: f64,
    /// Fraction of the total compared distance driven in this group.
    pub share: f64,
    pub windows: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SeasonalStats {
    pub by_tires: Vec<GroupStat>,
    pub by_season: Vec<GroupStat>,
    pub by_style: Vec<GroupStat>,
    pub by_roads: Vec<GroupStat>,
}

pub fn compute(points: &[ConsumptionPoint]) -> SeasonalStats {
    SeasonalStats {
        by_tires: group_by(points, |p| {
            Some(match p.tires? {
                Tires::Summer => "tires_summer",
                Tires::Winter => "tires_winter",
                Tires::AllYear => "tires_all_year",
            })
        }),
        by_season: group_by(points, |p| {
            Some(match p.date.month() {
                12 | 1 | 2 => "season_winter",
                3..=5 => "season_spring",
                6..=8 => "season_summer",
                _ => "season_autumn",
            })
        }),
        by_style: group_by(points, |p| {
            Some(match p.style? {
                DrivingStyle::Moderate => "style_moderate",
                DrivingStyle::Normal => "style_normal",
                DrivingStyle::Fast => "style_fast",
            })
        }),
        by_roads: roads_groups(points),
    }
}

fn group_by(
    points: &[ConsumptionPoint],
    key_of: impl Fn(&ConsumptionPoint) -> Option<&'static str>,
) -> Vec<GroupStat> {
    let mut groups = std::collections::BTreeMap::<&'static str, Vec<ConsumptionPoint>>::new();
    for p in points {
        if let Some(key) = key_of(p) {
            groups.entry(key).or_default().push(*p);
        }
    }
    build_stats(groups)
}

/// Road types are bitflags: a window tagged motorway+city contributes its
/// full distance to both groups. Honest given the data — the export doesn't
/// say how the distance was split.
fn roads_groups(points: &[ConsumptionPoint]) -> Vec<GroupStat> {
    let mut groups = std::collections::BTreeMap::<&'static str, Vec<ConsumptionPoint>>::new();
    for p in points {
        if p.roads.motorway() {
            groups.entry("roads_motorway").or_default().push(*p);
        }
        if p.roads.city() {
            groups.entry("roads_city").or_default().push(*p);
        }
        if p.roads.country() {
            groups.entry("roads_country").or_default().push(*p);
        }
    }
    build_stats(groups)
}

fn build_stats(
    groups: std::collections::BTreeMap<&'static str, Vec<ConsumptionPoint>>,
) -> Vec<GroupStat> {
    let total_distance: f64 = groups
        .values()
        .flatten()
        .map(|p| p.distance_km)
        .sum();
    let mut stats: Vec<GroupStat> = groups
        .into_iter()
        .filter_map(|(key, points)| {
            let distance_km: f64 = points.iter().map(|p| p.distance_km).sum();
            Some(GroupStat {
                key,
                liters_per_100km: weighted_average(&points)?,
                distance_km,
                share: if total_distance > 0.0 {
                    distance_km / total_distance
                } else {
                    0.0
                },
                windows: points.len(),
            })
        })
        .collect();
    // Most-driven group first — the reader's reference point.
    stats.sort_by(|a, b| b.distance_km.total_cmp(&a.distance_km));
    stats
}
