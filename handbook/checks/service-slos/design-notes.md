# Service SLOs — Design Notes

> ⚠️ **Draft/hypothesis.** This is early exploration. The final shape may
> look very different, or this check may not be feasible as described.

## The problem

Software teams define SLOs but have no way to evaluate them as part of
their regular development workflow. SLO tooling lives in production
monitoring (Sloth, Pyrra, Datadog, Grafana). Code quality tooling lives
in linters and CI.

Scute already evaluates fitness functions at Develop, Commit, and
Integrate. The `service-slos` check extends that into Deliver, Release,
and Observe: the same engine, the same contract, applied to runtime
characteristics.

## The model: service > CUJ > indicator

Following the Google SRE approach, SLOs start from Critical User Journeys
(CUJs), not raw metrics. A CUJ is something a user does (search, checkout,
upload). Each CUJ has indicators (availability, latency, correctness).
Each indicator has an SLI (how to measure), an objective (the target), and
a time window.

The config hierarchy reflects this:

```yaml
checks:
  service-slos:
    [service]:
      [cuj]:
        [indicator]:
          sli: ...
          objective: ...
          window: ...
```

### Why this structure

We evaluated four options:

1. **One check, all SLOs** — `service-slos` with service > CUJ > indicator
   hierarchy in the config.
2. **One check per service** — `slo-web-shop`, `slo-payments`, etc. Needs
   a `type` field, loses top-level grouping.
3. **One check per objective** — `slo-web-shop-search-latency`, etc.
   Flattest, but loses all hierarchy and needs metadata fields to
   reconstruct what the structure gave for free.
4. **Services as a top-level config concept** — A `services:` section
   alongside `checks:` in the config. Cleanest domain model, but bakes
   service/SLO knowledge into the engine's config schema. Moves away from
   Scute's generalist check-engine approach.

Leaning toward option 1. The structure _is_ the metadata. No extra fields
needed. One additional nesting level is a trivial cost for clean grouping
and natural CLI drill-down. Option 4 was elegant but risks coupling the
engine to a specific domain model.

## Config example

```yaml
checks:
  service-slos:
    web-shop:
      search:
        availability:
          sli: ratio(successful_searches, total_searches)
          objective: 99.9
          window: 30d
        latency:
          sli: p99(search_duration)
          objective: 200ms
          window: 30d
    payments:
      checkout:
        availability:
          sli: ratio(successful_checkouts, total_checkouts)
          objective: 99.99
          window: 30d
```

## CLI

Natural drill-down at every level:

```sh
scute check service-slos                              # all services
scute check service-slos web-shop                     # all CUJs for web-shop
scute check service-slos web-shop search              # all indicators for search
scute check service-slos web-shop search latency      # one specific SLO
```

## SLI types

Following Google SRE, every SLI is a ratio: `good events / valid events`,
always producing a value between 0% and 100%.

**Request/Response SLIs:**

| Type | Good event | What determines good/bad |
|---|---|---|
| Availability | Non-error response | Status code classification |
| Latency | Response faster than threshold | Duration vs. time threshold |
| Quality | Non-degraded response | App-level degradation flag |

**Data Processing SLIs:**

| Type | Good event | What determines good/bad |
|---|---|---|
| Freshness | Record newer than threshold | Record age vs. staleness threshold |
| Correctness | Output matches expected | Comparison against golden dataset |
| Coverage | Record was processed | Processed count vs. expected count |
| Throughput | Time unit met rate threshold | Processing rate vs. rate threshold |

Two measurement modes: **request-based** (good requests / total requests)
and **window-based** (good time intervals / total time intervals, used
when only aggregates like p99 are available).

## Objectives are static

Following Google SRE best practice, SLO targets are static. The objective
doesn't change based on load, traffic, or time of day.

This means Scute's existing warn/fail threshold model can apply directly
to SLO ratios. The `fail` threshold is the objective. The `warn` threshold
is the early signal buffer. Both are static, and that's by design.

## Open questions

- **SLI data source.** Where does the measured value come from? A
  Prometheus query, a user-provided command, piped input? The best
  dev/agent experience is `scute check service-slos` and Scute handles
  the rest, but how the data gets in is still open.
- **Error budgets.** SLOs imply error budgets. Error budget evaluation
  would be a separate check (different thing measured, different action
  triggered). How it relates to `service-slos` is still open.
- **Config example gap.** The config examples use `sli: ratio(...)` and
  `sli: p99(...)` as placeholders. The actual SLI definition syntax
  (how to express the measurement formula and its data source) needs
  design work.
