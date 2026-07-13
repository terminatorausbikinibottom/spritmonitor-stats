//! Chart payloads for the vendored uPlot frontend.
//!
//! Rust decides *what* is plotted; the ~30 lines of glue in `app.js` decide
//! *how*. Each chart is serialized into a `data-chart` attribute.

use serde::Serialize;

use crate::i18n::Lang;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chart {
    /// "line" or "bars".
    pub kind: &'static str,
    /// "time" (x = unix seconds) or "ordinal" (x = index into labels).
    pub x_type: &'static str,
    pub x: Vec<f64>,
    /// Category labels for ordinal charts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    pub series: Vec<Series>,
    pub unit: String,
    pub locale: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Series {
    pub label: String,
    pub values: Vec<Option<f64>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dashed: bool,
}

impl Chart {
    pub fn time(lang: Lang, unit: impl Into<String>) -> Self {
        Self {
            kind: "line",
            x_type: "time",
            x: Vec::new(),
            labels: Vec::new(),
            series: Vec::new(),
            unit: unit.into(),
            locale: lang.code(),
        }
    }

    pub fn ordinal_bars(lang: Lang, unit: impl Into<String>, labels: Vec<String>) -> Self {
        Self {
            kind: "bars",
            x_type: "ordinal",
            x: (0..labels.len()).map(|i| i as f64).collect(),
            labels,
            series: Vec::new(),
            unit: unit.into(),
            locale: lang.code(),
        }
    }

    pub fn with_x(mut self, x: Vec<f64>) -> Self {
        self.x = x;
        self
    }

    pub fn with_series(mut self, label: impl Into<String>, values: Vec<Option<f64>>) -> Self {
        self.series.push(Series {
            label: label.into(),
            values,
            dashed: false,
        });
        self
    }

    pub fn with_dashed_series(
        mut self,
        label: impl Into<String>,
        values: Vec<Option<f64>>,
    ) -> Self {
        self.series.push(Series {
            label: label.into(),
            values,
            dashed: true,
        });
        self
    }

    /// JSON for the `data-chart` attribute. Never fails: the struct contains
    /// only serializable primitives.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("chart serialization cannot fail")
    }
}
