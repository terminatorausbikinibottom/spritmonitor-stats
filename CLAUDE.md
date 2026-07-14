# CLAUDE.md

Self-hosted web dashboard that parses a Spritmonitor.de CSV export in memory and shows vehicle
statistics. The headline feature is yearly-mileage estimation for car insurance (the user doesn't
want to overpay for unused mileage). No database, no disk writes, no external requests.

## Workflow

- **Do not run `cargo` (build/test/run) unless explicitly asked** — the user builds and verifies
  himself.
- Module layout is `foo.rs` + `foo/` subdirectory — **never `mod.rs`**.
- Binary name: `spritmonitor-stats`. Container builds use **podman**, `FROM scratch`, static musl.

## Architecture (data flows left to right)

```
importer  →  domain::Dataset  →  analytics::StatsReport  →  web/views + templates
```

Layers are strictly separated:

- `src/importer/csv.rs` — all CSV messiness lives here: semicolon/comma delimiters, decimal commas,
  German/English headers, Windows-1252 fallback, preamble skipping. Row-level problems become
  `ImportWarning`s inside a successful dataset; only structural failure is a `SourceError`.
  `importer.rs` defines the `DatasetSource` trait (a future spritmonitor.de scraper would be a
  second impl).
- `src/domain.rs` — canonical model, knows nothing about CSV/HTTP/rendering. `Dataset::new` sorts
  fuelings, reconstructs missing odometers from trip distances, discards non-monotonic readings.
  `FuelingKind`: 0 invalid, 1 full, 2 partial, 3 first. AdBlue (fuel sort 21) is excluded from
  consumption/mileage, reported separately in costs.
- `src/analytics/` — **pure**: datasets and params in, plain structs out. "Today" is always passed
  in (`StatsParams.today`), never read from a clock, so results are test-reproducible. All mileage
  math builds on `timeline::OdometerTimeline` (interpolated odometer readings). `anchor_date`
  clamps "today" to the last reading so abandoned exports don't produce zeros.
  - `mileage.rs` — the insurance feature: `insurance_projection` (policy-anniversary year, pace +
    trailing-90-day projections, past-year history), `trailing_12_months`, `calendar_years`,
    OLS `forecast` with r², `monthly_km`.
  - `consumption.rs` — consumption windows close only at *full* fuelings; partials roll forward.
  - `seasonal.rs`, `costs.rs`, `behavior.rs` — comparison tables, spend, habits.
- `src/web/` — axum 0.8. Routes in `routes.rs`; askama templates in `templates/` rendered via
  structs in `views.rs` (all formatting happens in Rust — templates stay dumb). htmx drives the
  interactive panels via `GET /partials/insurance?anniversary=` and `/partials/forecast?months=`.
  `session.rs`: uploaded datasets live in a moka cache keyed by `sid` cookie; the `SessionDataset`
  extractor rejects with a localized session-expired response (htmx-aware). Assets (htmx, uPlot,
  CSS, JS) are `include_bytes!`-embedded in `assets.rs`.
- `src/charts.rs` — serializes chart payloads into `data-chart` attributes; `assets/app.js` (~30
  lines of glue) renders them with vendored uPlot.
- `src/i18n.rs` + `locales/app.yml` — rust-i18n, `de` (default) and `en`, `_version: 2` YAML
  format. Every user-visible string goes through `Tr::m("key")`; number/date formatting is
  locale-aware in Rust (`fmt_num`, `fmt_date`, `fmt_money`).

## Conventions

- Parsing uses `rust_decimal::Decimal`; analytics converts to `f64`.
- Dates are `jiff::civil::Date` throughout.
- Config is env-vars-only, read once at startup in `src/config.rs` (`BIND_ADDR`,
  `MAX_UPLOAD_BYTES`, `SESSION_TTL_SECS`, `SESSION_CAPACITY`, `DEFAULT_LANG`).
- Tests are inline `#[cfg(test)]` modules (currently in `importer/csv.rs`, `i18n.rs`);
  `tests/fixtures/` exists for future integration tests. Analytics is deliberately clock-free to
  keep it testable.
- When adding a UI string: add the key to `locales/app.yml` with both `en:` and `de:` values.
