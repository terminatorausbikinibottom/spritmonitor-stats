// Drop-zone behavior and uPlot chart initialization.
// Charts are declared server-side as <div class="chart" data-chart="{json}">.
"use strict";

// ---- upload drop zone -----------------------------------------------------

function initDropzone() {
  const zone = document.getElementById("dropzone");
  const input = document.getElementById("file-input");
  const form = document.getElementById("upload-form");
  if (!zone || !input || !form) return;

  ["dragenter", "dragover"].forEach((name) =>
    zone.addEventListener(name, (e) => {
      e.preventDefault();
      zone.classList.add("dragover");
    })
  );
  ["dragleave", "drop"].forEach((name) =>
    zone.addEventListener(name, (e) => {
      e.preventDefault();
      zone.classList.remove("dragover");
    })
  );
  zone.addEventListener("drop", (e) => {
    if (e.dataTransfer.files.length > 0) {
      input.files = e.dataTransfer.files;
      form.submit();
    }
  });
  input.addEventListener("change", () => {
    if (input.files.length > 0) form.submit();
  });
}

// ---- charts ---------------------------------------------------------------

const SERIES_COLORS = ["#1f6feb", "#e8874a"];

function fmtValue(value, locale) {
  if (value == null) return "–";
  return value.toLocaleString(locale === "de" ? "de-DE" : "en-US");
}

function buildChart(el) {
  const cfg = JSON.parse(el.dataset.chart);
  if (el._uplot) {
    el._uplot.destroy();
    el._uplot = null;
  }
  el.textContent = "";
  if (!cfg.x || cfg.x.length === 0) return;

  const isBars = cfg.kind === "bars";
  const series = [
    {
      value: (u, raw) =>
        cfg.xType === "time"
          ? new Date(raw * 1000).toLocaleDateString(cfg.locale === "de" ? "de-DE" : "en-US")
          : (cfg.labels || [])[raw] ?? raw,
    },
  ];
  cfg.series.forEach((s, i) => {
    series.push({
      label: s.label,
      stroke: SERIES_COLORS[i % SERIES_COLORS.length],
      width: 2,
      dash: s.dashed ? [6, 4] : undefined,
      fill: isBars ? SERIES_COLORS[i % SERIES_COLORS.length] + "cc" : undefined,
      paths: isBars ? uPlot.paths.bars({ size: [0.6, 100] }) : undefined,
      points: { show: !isBars && cfg.x.length <= 60 },
      value: (u, raw) => `${fmtValue(raw, cfg.locale)} ${cfg.unit}`,
    });
  });

  const opts = {
    width: el.clientWidth || 600,
    height: 240,
    series,
    scales: { x: { time: cfg.xType === "time" } },
    axes: [
      cfg.xType === "ordinal"
        ? {
            values: (u, splits) =>
              splits.map((v) =>
                Number.isInteger(v) ? (cfg.labels || [])[v] ?? "" : ""
              ),
          }
        : {},
      { values: (u, splits) => splits.map((v) => fmtValue(v, cfg.locale)) },
    ],
    legend: { show: cfg.series.length > 1 },
    cursor: { drag: { x: false, y: false } },
  };

  const data = [cfg.x, ...cfg.series.map((s) => s.values)];
  el._uplot = new uPlot(opts, data, el);
}

function renderCharts(root) {
  const scope = root instanceof Element ? root : document;
  const charts = scope.querySelectorAll("[data-chart]");
  charts.forEach(buildChart);
  if (scope !== document && scope.matches("[data-chart]")) buildChart(scope);
}

let resizeTimer;
window.addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => renderCharts(document), 150);
});

document.addEventListener("DOMContentLoaded", () => {
  initDropzone();
  renderCharts(document);
});

// Re-render charts inside fragments swapped in by HTMX.
document.addEventListener("htmx:afterSwap", (e) => renderCharts(e.target));
