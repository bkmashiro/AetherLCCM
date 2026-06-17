# Simulation scaffold

## Modules
- `scenario.py`
  - deterministic DSL parser for domains, distances, events, messages, claims, checkpoints, policy, offline windows, and expectations.
- `scheduler.py`
  - deterministic event/message scheduling stub.
- `verifier.py`
  - invariant verification hooks.
- `models.py`
  - core simulation types mirroring protocol model concepts.
- `__main__.py`
  - runnable entrypoint for quick smoke validation.

## Planned scenario coverage
- Honest cross-system settlement
- Relay-delayed old checkpoint replay
- Double lock conflict
- Byzantine conflict reporting
- Offline ship rejoin
- Lightcone-impossible message rejection

## Phase 6 hardening additions
- Bounded-route and bounded-delay checks for message transport
- Offline-domain window modeling for resync-aware expectations
- Deterministic trace export (`--trace-json`) from scheduler runs
- New machine-readable outputs:
  - `--trace-json` for replay/audit traces
  - `--tla-json` for TLA+-consumable deterministic state steps
  - `--causal-json` for causal graph (nodes + edges)
  - `--alloy-json` for Alloy-style dependency payload
- New exports are used by phase-6 hardening tests for formal-flow integration.
