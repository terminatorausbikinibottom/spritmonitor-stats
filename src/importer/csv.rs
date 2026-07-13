//! Importer for Spritmonitor.de CSV exports.
//!
//! Real-world exports vary: semicolon or comma delimiters, decimal commas,
//! German or English headers, Windows-1252 encoding, occasional preamble
//! lines. All of that messiness is contained here.

use std::borrow::Cow;

use jiff::civil::Date;
use rust_decimal::Decimal;

use crate::domain::{
    Dataset, DrivingStyle, Fueling, FuelingKind, FuelSort, ImportWarning, Roads, Tires,
    WarningKind,
};
use crate::importer::{DatasetSource, SourceError};

/// Imports a Spritmonitor CSV export from raw file bytes.
pub struct CsvSource {
    bytes: Vec<u8>,
}

impl CsvSource {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl DatasetSource for CsvSource {
    async fn fetch(&self) -> Result<Dataset, SourceError> {
        parse(&self.bytes)
    }
}

/// The columns we know how to interpret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Column {
    Date,
    Odometer,
    TripDistance,
    Quantity,
    TotalPrice,
    Currency,
    Kind,
    Tires,
    Roads,
    Style,
    FuelSort,
    Note,
    Consumption,
    BcConsumption,
    BcQuantity,
    BcSpeed,
    Company,
    Country,
    Area,
    Location,
}

fn parse(bytes: &[u8]) -> Result<Dataset, SourceError> {
    let mut warnings = Vec::new();
    let text = decode(bytes, &mut warnings);

    let header = find_header(&text).ok_or(SourceError::NoRecognizableHeader)?;
    if header.line_index > 0 {
        warnings.push(ImportWarning {
            line: None,
            kind: WarningKind::PreambleSkipped,
        });
    }
    for name in &header.unknown_columns {
        warnings.push(ImportWarning {
            line: Some(header.line_index + 1),
            kind: WarningKind::UnknownColumn(name.clone()),
        });
    }

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(header.delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(text[header.byte_offset..].as_bytes());

    let mut fuelings = Vec::new();
    for (index, record) in reader.records().enumerate().skip(1) {
        // 1-based line number in the original file; good enough as long as
        // fields contain no embedded newlines, which Spritmonitor's don't.
        let line = header.line_index + 1 + index;
        let record = match record {
            Ok(r) => r,
            Err(e) => return Err(SourceError::Csv(e.to_string())),
        };
        if record.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        if let Some(fueling) = parse_row(&record, &header.columns, line, &mut warnings) {
            fuelings.push(fueling);
        }
    }

    if fuelings.is_empty() {
        return Err(SourceError::NoValidRows);
    }
    Ok(Dataset::new(fuelings, warnings))
}

fn decode<'a>(bytes: &'a [u8], warnings: &mut Vec<ImportWarning>) -> Cow<'a, str> {
    let without_bom = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    match std::str::from_utf8(without_bom) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => {
            warnings.push(ImportWarning {
                line: None,
                kind: WarningKind::Windows1252Fallback,
            });
            let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(without_bom);
            Cow::Owned(decoded.into_owned())
        }
    }
}

struct Header {
    /// 0-based index of the header line.
    line_index: usize,
    /// Byte offset of the header line within the decoded text.
    byte_offset: usize,
    delimiter: u8,
    /// Column meaning by field index; `None` for ignored columns.
    columns: Vec<Option<Column>>,
    unknown_columns: Vec<String>,
}

/// Scans the first lines for one that contains at least three recognizable
/// column names. Spritmonitor exports start with the header directly, but a
/// stray preamble line must not break the import.
fn find_header(text: &str) -> Option<Header> {
    for (line_index, line) in text.lines().take(10).enumerate() {
        // lines() yields subslices of `text`, so pointer arithmetic gives the
        // exact byte offset regardless of \n vs \r\n endings.
        let byte_offset = line.as_ptr() as usize - text.as_ptr() as usize;
        if let Some(header) = try_header_line(line, line_index, byte_offset) {
            return Some(header);
        }
    }
    None
}

fn try_header_line(line: &str, line_index: usize, byte_offset: usize) -> Option<Header> {
    let delimiter = *[b';', b',', b'\t']
        .iter()
        .max_by_key(|&&d| line.bytes().filter(|&b| b == d).count())?;
    if line.bytes().filter(|&b| b == delimiter).count() < 2 {
        return None;
    }

    let mut columns = Vec::new();
    let mut unknown = Vec::new();
    for cell in line.split(delimiter as char) {
        let cell = cell.trim().trim_matches('"').trim();
        let column = column_for(&normalize_header(cell));
        if column.is_none() && !cell.is_empty() {
            unknown.push(cell.to_string());
        }
        columns.push(column);
    }

    let known = columns.iter().flatten().count();
    let has_date = columns.contains(&Some(Column::Date));
    let has_data = [Column::Odometer, Column::TripDistance, Column::Quantity]
        .iter()
        .any(|c| columns.contains(&Some(*c)));
    (known >= 3 && has_date && has_data).then_some(Header {
        line_index,
        byte_offset,
        delimiter,
        columns,
        unknown_columns: unknown,
    })
}

/// Lowercases, transliterates umlauts and collapses everything that is not
/// alphanumeric into single spaces: "BC-Verbrauch" -> "bc verbrauch".
fn normalize_header(raw: &str) -> String {
    let lower = raw
        .to_lowercase()
        .replace('ä', "ae")
        .replace('ö', "oe")
        .replace('ü', "ue")
        .replace('ß', "ss");
    let mut out = String::with_capacity(lower.len());
    for c in lower.chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with(' ') && !out.is_empty() {
            out.push(' ');
        }
    }
    out.trim().to_string()
}

fn column_for(normalized: &str) -> Option<Column> {
    use Column::*;
    Some(match normalized {
        "date" | "datum" => Date,
        "odometer" | "mileage" | "kilometerstand" | "km stand" => Odometer,
        "trip" | "distance" | "strecke" | "trip distance" => TripDistance,
        "quantity" | "menge" | "amount" => Quantity,
        "total price" | "totalprice" | "gesamtpreis" | "price" | "preis" => TotalPrice,
        "currency" | "waehrung" => Currency,
        "type" | "fueling type" | "art" | "art des tankvorgangs" | "tankvorgang" => Kind,
        "tires" | "tyres" | "reifen" => Tires,
        "roads" | "strassen" | "streets" => Roads,
        "driving style" | "fahrweise" | "style" => Style,
        "fuel sort" | "fuel" | "fuel type" | "kraftstoffsorte" | "kraftstoff" => FuelSort,
        "note" | "notiz" | "bemerkung" => Note,
        "consumption" | "verbrauch" => Consumption,
        "bc consumption" | "bc verbrauch" => BcConsumption,
        "bc quantity" | "bc menge" => BcQuantity,
        "bc speed" | "bc geschwindigkeit" => BcSpeed,
        "company" | "firma" => Company,
        "country" | "land" => Country,
        "area" | "gebiet" | "region" => Area,
        "location" | "ort" | "city" => Location,
        _ => return None,
    })
}

fn parse_row(
    record: &csv::StringRecord,
    columns: &[Option<Column>],
    line: usize,
    warnings: &mut Vec<ImportWarning>,
) -> Option<Fueling> {
    let cell = |wanted: Column| -> Option<&str> {
        columns
            .iter()
            .position(|c| *c == Some(wanted))
            .and_then(|i| record.get(i))
            .map(|s| s.trim().trim_matches('"').trim())
            .filter(|s| !s.is_empty())
    };
    let text = |wanted: Column| cell(wanted).map(str::to_string);
    let mut decimal = |wanted: Column, name: &'static str| -> Option<Decimal> {
        let raw = cell(wanted)?;
        match parse_decimal(raw) {
            Some(v) => Some(v),
            None => {
                warnings.push(ImportWarning {
                    line: Some(line),
                    kind: WarningKind::BadValue {
                        column: name,
                        value: raw.to_string(),
                    },
                });
                None
            }
        }
    };

    let raw_date = cell(Column::Date).unwrap_or("");
    let Some(date) = parse_date(raw_date) else {
        warnings.push(ImportWarning {
            line: Some(line),
            kind: WarningKind::RowSkipped {
                value: raw_date.to_string(),
            },
        });
        return None;
    };

    let odometer = decimal(Column::Odometer, "odometer");
    let trip_distance = decimal(Column::TripDistance, "distance");
    let quantity = decimal(Column::Quantity, "quantity");
    let total_price = decimal(Column::TotalPrice, "total price");

    let mut code = |wanted: Column, name: &'static str| -> Option<u8> {
        let raw = cell(wanted)?;
        match raw.parse::<u8>() {
            Ok(v) => Some(v),
            Err(_) => {
                warnings.push(ImportWarning {
                    line: Some(line),
                    kind: WarningKind::BadValue {
                        column: name,
                        value: raw.to_string(),
                    },
                });
                None
            }
        }
    };

    let kind = code(Column::Kind, "fueling type")
        .and_then(FuelingKind::from_code)
        // A missing type column happens on hand-edited files; assuming full
        // fuelings keeps consumption computable.
        .unwrap_or(FuelingKind::Full);
    let tires = code(Column::Tires, "tires").and_then(Tires::from_code);
    let roads = code(Column::Roads, "roads")
        .map(Roads::from_code)
        .unwrap_or_default();
    let style = code(Column::Style, "driving style").and_then(DrivingStyle::from_code);
    let fuel_sort = code(Column::FuelSort, "fuel sort").map(FuelSort);

    Some(Fueling {
        date,
        odometer,
        trip_distance,
        quantity,
        total_price,
        currency: text(Column::Currency),
        kind,
        tires,
        roads,
        style,
        fuel_sort,
        note: text(Column::Note),
        company: text(Column::Company),
        country: text(Column::Country),
        area: text(Column::Area),
        location: text(Column::Location),
        source_line: line,
    })
}

/// Parses a decimal that may use either `,` or `.` as the decimal separator.
/// If both occur the last one is the decimal point and the other is stripped
/// as a thousands separator (defensive — the export spec promises none).
fn parse_decimal(raw: &str) -> Option<Decimal> {
    let s = raw.trim();
    let comma = s.rfind(',');
    let point = s.rfind('.');
    let cleaned: String = match (comma, point) {
        (Some(c), Some(p)) if c > p => s.replace('.', "").replace(',', "."),
        (Some(_), Some(_)) => s.replace(',', ""),
        (Some(_), None) => s.replace(',', "."),
        _ => s.to_string(),
    };
    cleaned.parse().ok()
}

fn parse_date(raw: &str) -> Option<Date> {
    for format in ["%d.%m.%Y", "%Y-%m-%d", "%d.%m.%y"] {
        if let Ok(date) = Date::strptime(format, raw) {
            return Some(date);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_variants() {
        let dec = |s: &str| Decimal::from_str_exact(s).unwrap();
        assert_eq!(parse_decimal("1234,5"), Some(dec("1234.5")));
        assert_eq!(parse_decimal("1234.5"), Some(dec("1234.5")));
        assert_eq!(parse_decimal("1.234,5"), Some(dec("1234.5")));
        assert_eq!(parse_decimal("1,234.5"), Some(dec("1234.5")));
        assert_eq!(parse_decimal("42"), Some(dec("42")));
        assert_eq!(parse_decimal("abc"), None);
    }

    #[test]
    fn header_normalization() {
        assert_eq!(normalize_header("BC-Verbrauch"), "bc verbrauch");
        assert_eq!(normalize_header("  Straßen "), "strassen");
        assert_eq!(normalize_header("Total price"), "total price");
        assert_eq!(normalize_header("\"Währung\""), "waehrung");
    }

    #[test]
    fn date_variants() {
        let expected = jiff::civil::date(2010, 2, 23);
        assert_eq!(parse_date("23.02.2010"), Some(expected));
        assert_eq!(parse_date("2010-02-23"), Some(expected));
        assert_eq!(parse_date("not a date"), None);
    }
}
