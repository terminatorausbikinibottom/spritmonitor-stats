# Spritmonitor Statistics

A small, self-hosted web dashboard that turns a [Spritmonitor.de](https://www.spritmonitor.de) CSV
export into vehicle statistics — most importantly, an **estimate of your yearly mileage for your car
insurance**.

> [!WARNING]
> ## 🤖 Vibe coded, human guided
> This project was **written almost entirely by an AI coding assistant**, with a human steering the
> requirements, reviewing the direction, and verifying the results. It works for its author's data
> and use case, but it has not been engineered, audited, or hardened the way a hand-written
> production project would be. Read the code before you trust it, self-host it only for yourself,
> and double-check any number you send to your insurance company.

## Why this exists

Spritmonitor is great for tracking real mileage and real fuel consumption. One thing it doesn't
offer (or hides really well) is an answer to the question every car insurance asks once a year:

> *"How many kilometers will you drive in the next insurance year?"*

Guessing too high means paying for mileage you never drive. Guessing too low means an awkward
adjustment later. This tool takes the fueling history you already maintain on Spritmonitor and
answers the question with data:

- **Insurance-year projection** — enter your policy anniversary date, get the kilometers driven so
  far in the current insurance year plus two projections for where you'll land: one at the average
  pace of the year so far, one that weighs the last 90 days more heavily (reacts faster if your
  habits changed). Completed past insurance years are listed for comparison.
- **Trailing 12 months** — kilometers driven in the last 365 days, the simplest honest answer.
- **Calendar-year history** — kilometers per calendar year, annualized for partially covered years.
- **12-month forecast** — a linear-regression forecast over a configurable window, with r² so you
  can judge how steady your driving actually is.

Everything is derived from an interpolated odometer timeline, so the math tolerates missing trip
distances, irregular fueling intervals, and gaps in the data.

## What else it shows

Since the CSV is already parsed, the dashboard also covers:

- **Consumption** — computed correctly across partial fuelings (consumption is only well-defined at
  a *full* fueling; partial fills roll into the next full one), with trend charts.
- **Costs** — total fuel and AdBlue spend, cost per kilometer, price trends.
- **Seasonal & behavioral comparisons** — consumption by tire type (summer/winter/all-year), season,
  road type (city/country/motorway), and driving style.
- **Fueling habits** — how often, how much, how far between fuelings.
- **Import warnings** — anything odd in your export (unreadable rows, odometer readings that go
  backwards, reconstructed values) is surfaced instead of silently swallowed.

The UI is available in **German and English** (cookie → `Accept-Language` → configured default).

## Privacy

- Your CSV is parsed **in memory only**. Nothing is written to disk, no database exists.
- The parsed dataset lives in an in-memory session cache (default: 2 h idle TTL, 100 sessions) so
  the interactive controls (anniversary date, forecast window) don't re-upload the file. Restarting
  the server wipes everything.
- All assets (htmx, uPlot, CSS) are embedded in the binary — the app makes **zero external
  requests** and works fully offline.

## Getting your data

On spritmonitor.de: **your vehicle → Fuelings → CSV export**. Drop the file onto the start page.
The importer copes with real-world export messiness: semicolon or comma delimiters, decimal commas,
German or English headers, Windows-1252 encoding, and preamble lines.

## Running it

### With Docker / Podman

```sh
podman build -t spritmonitor-stats .
podman run --rm -p 8080:8080 spritmonitor-stats
```

The image builds a fully static musl binary and ships it in a `FROM scratch` image — the final
container contains a single file.

### From source

```sh
cargo run --release
# listening on http://0.0.0.0:8080
```

### Configuration

Everything is configured via environment variables; all of them are optional.

| Variable           | Default        | Meaning                                        |
| ------------------ | -------------- | ---------------------------------------------- |
| `BIND_ADDR`        | `0.0.0.0:8080` | Listen address                                 |
| `MAX_UPLOAD_BYTES` | `5242880`      | Upload size limit (5 MiB)                      |
| `SESSION_TTL_SECS` | `7200`         | Idle lifetime of an uploaded dataset           |
| `SESSION_CAPACITY` | `100`          | Max concurrent sessions kept in memory         |
| `DEFAULT_LANG`     | `de`           | Fallback UI language (`de` or `en`)            |

`GET /healthz` returns `ok` for container health checks.

## Tech stack

Rust · [axum](https://github.com/tokio-rs/axum) · [askama](https://github.com/askama-rs/askama)
templates · [htmx](https://htmx.org) for the interactive panels ·
[uPlot](https://github.com/leeoniya/uPlot) for charts · [jiff](https://github.com/BurntSushi/jiff)
for dates · [moka](https://github.com/moka-rs/moka) for the session cache. No database, no JS build
step, no external services.

The analytics layer is pure (datasets and parameters in, plain structs out — even "today" is passed
in, never read from a clock), which keeps every figure unit-testable and reproducible.

## Disclaimer, again

This is a **vibe coded, human guided** hobby project. It is not affiliated with, endorsed by, or
connected to Spritmonitor.de in any way. Numbers it produces are estimates based on your own data —
sanity-check them before making financial decisions.

## License

MIT
