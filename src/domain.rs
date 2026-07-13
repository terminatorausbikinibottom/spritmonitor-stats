//! Core domain model shared by all layers.
//!
//! This module has no knowledge of CSV files, HTTP or rendering. Importers
//! produce a [`Dataset`], analytics consume it.

use jiff::civil::Date;
use rust_decimal::Decimal;

/// One fueling entry, normalized from whatever source produced it.
#[derive(Debug, Clone)]
pub struct Fueling {
    pub date: Date,
    /// Total odometer reading in km.
    pub odometer: Option<Decimal>,
    /// Distance since the previous fueling in km.
    pub trip_distance: Option<Decimal>,
    /// Fueled quantity in liters.
    pub quantity: Option<Decimal>,
    pub total_price: Option<Decimal>,
    pub currency: Option<String>,
    pub kind: FuelingKind,
    pub tires: Option<Tires>,
    pub roads: Roads,
    pub style: Option<DrivingStyle>,
    pub fuel_sort: Option<FuelSort>,
    pub note: Option<String>,
    pub company: Option<String>,
    pub country: Option<String>,
    pub area: Option<String>,
    pub location: Option<String>,
    /// 1-based line number in the source file, for warnings.
    pub source_line: usize,
}

/// Spritmonitor fueling type codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuelingKind {
    /// 0 — marked invalid by the user; excluded from all statistics.
    Invalid,
    /// 1 — tank filled completely; consumption windows close here.
    Full,
    /// 2 — partial fueling; quantity rolls into the next full fueling.
    Partial,
    /// 3 — first fueling; establishes the odometer baseline.
    First,
}

impl FuelingKind {
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Invalid),
            1 => Some(Self::Full),
            2 => Some(Self::Partial),
            3 => Some(Self::First),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tires {
    Summer,
    Winter,
    AllYear,
}

impl Tires {
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Summer),
            2 => Some(Self::Winter),
            3 => Some(Self::AllYear),
            _ => None,
        }
    }
}

/// Road types of a trip, encoded by Spritmonitor as a bitflag sum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Roads(u8);

impl Roads {
    pub const MOTORWAY: u8 = 2;
    pub const CITY: u8 = 4;
    pub const COUNTRY: u8 = 8;

    pub fn from_code(code: u8) -> Self {
        Self(code & (Self::MOTORWAY | Self::CITY | Self::COUNTRY))
    }

    pub fn motorway(self) -> bool {
        self.0 & Self::MOTORWAY != 0
    }

    pub fn city(self) -> bool {
        self.0 & Self::CITY != 0
    }

    pub fn country(self) -> bool {
        self.0 & Self::COUNTRY != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrivingStyle {
    Moderate,
    Normal,
    Fast,
}

impl DrivingStyle {
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Moderate),
            2 => Some(Self::Normal),
            3 => Some(Self::Fast),
            _ => None,
        }
    }
}

/// Spritmonitor fuel sort code. Kept as a newtype rather than a closed enum —
/// the site adds new codes over time and unknown ones must not break imports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuelSort(pub u8);

impl FuelSort {
    /// AdBlue is an additive, not a fuel: excluded from consumption and
    /// mileage statistics, shown separately in costs.
    pub fn is_adblue(self) -> bool {
        self.0 == 21
    }

    pub fn label(self) -> &'static str {
        match self.0 {
            1 => "Diesel",
            2 => "Biodiesel",
            3 => "Vegetable oil",
            4 => "Premium Diesel",
            6 => "Normal gasoline",
            7 => "Super gasoline",
            8 => "SuperPlus gasoline",
            9 => "Premium Gasoline 100",
            12 => "LPG",
            13 => "CNG H",
            14 => "CNG L",
            15 => "Bio-alcohol",
            16 => "Two-stroke",
            18 => "Premium Gasoline 95",
            19 => "Electricity",
            20 => "E10",
            21 => "AdBlue",
            22 => "Premium Gasoline 100+",
            23 => "Hydrogen",
            24 => "Green electricity",
            25 => "GTL Diesel",
            26 => "HVO100",
            _ => "Other",
        }
    }
}

/// A non-fatal problem encountered while importing. The import succeeds;
/// warnings are shown to the user so they can fix their data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportWarning {
    /// 1-based line number in the source file, when attributable.
    pub line: Option<usize>,
    pub kind: WarningKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarningKind {
    /// Input was not valid UTF-8; decoded as Windows-1252 instead.
    Windows1252Fallback,
    /// Lines before the recognized header row were ignored.
    PreambleSkipped,
    /// A column header that maps to no known field; the column was ignored.
    UnknownColumn(String),
    /// The whole row was dropped because its date is missing or unreadable.
    RowSkipped { value: String },
    /// A single field was unreadable and treated as empty.
    BadValue { column: &'static str, value: String },
    /// Odometer was missing and reconstructed from the trip distance.
    OdometerReconstructed,
    /// Odometer went backwards; the reading was discarded (row kept).
    OdometerNonMonotonic,
}

/// The complete fueling history of one vehicle, sorted by date.
#[derive(Debug, Clone)]
pub struct Dataset {
    fuelings: Vec<Fueling>,
    /// Dominant currency across entries, e.g. "EUR".
    pub currency: Option<String>,
    pub warnings: Vec<ImportWarning>,
}

impl Dataset {
    /// Builds a dataset from raw fuelings: sorts them, reconstructs missing
    /// odometer readings from trip distances and discards readings that go
    /// backwards, so the odometer sequence is usable as a timeline.
    pub fn new(mut fuelings: Vec<Fueling>, mut warnings: Vec<ImportWarning>) -> Self {
        fuelings.sort_by(|a, b| a.date.cmp(&b.date).then(a.odometer.cmp(&b.odometer)));

        // Reconstruct missing odometer values where the trip distance allows it.
        let mut prev_odometer: Option<Decimal> = None;
        for f in &mut fuelings {
            if f.odometer.is_none() {
                if let (Some(prev), Some(dist)) = (prev_odometer, f.trip_distance) {
                    f.odometer = Some(prev + dist);
                    warnings.push(ImportWarning {
                        line: Some(f.source_line),
                        kind: WarningKind::OdometerReconstructed,
                    });
                }
            }
            if f.odometer.is_some() {
                prev_odometer = f.odometer;
            }
        }

        // Discard odometer readings that go backwards in time.
        let mut max_odometer: Option<Decimal> = None;
        for f in &mut fuelings {
            match (f.odometer, max_odometer) {
                (Some(o), Some(max)) if o < max => {
                    f.odometer = None;
                    warnings.push(ImportWarning {
                        line: Some(f.source_line),
                        kind: WarningKind::OdometerNonMonotonic,
                    });
                }
                (Some(o), _) => max_odometer = Some(o),
                _ => {}
            }
        }

        let currency = dominant_currency(&fuelings);
        Self {
            fuelings,
            currency,
            warnings,
        }
    }

    /// All entries, sorted by date.
    pub fn fuelings(&self) -> &[Fueling] {
        &self.fuelings
    }

    /// Rows relevant to fuel statistics: valid and not an additive (AdBlue).
    pub fn fuel_rows(&self) -> impl Iterator<Item = &Fueling> {
        self.fuelings
            .iter()
            .filter(|f| f.kind != FuelingKind::Invalid)
            .filter(|f| !f.fuel_sort.is_some_and(FuelSort::is_adblue))
    }

    /// Valid AdBlue purchases, reported separately in cost statistics.
    pub fn adblue_rows(&self) -> impl Iterator<Item = &Fueling> {
        self.fuelings
            .iter()
            .filter(|f| f.kind != FuelingKind::Invalid)
            .filter(|f| f.fuel_sort.is_some_and(FuelSort::is_adblue))
    }

    pub fn is_empty(&self) -> bool {
        self.fuelings.is_empty()
    }
}

fn dominant_currency(fuelings: &[Fueling]) -> Option<String> {
    let mut counts = std::collections::HashMap::<&str, usize>::new();
    for c in fuelings.iter().filter_map(|f| f.currency.as_deref()) {
        *counts.entry(c).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(c, _)| c.to_string())
}
