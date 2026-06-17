# protocol crate scaffolding

## Modules
- `api.rs`
  - Canonical entry points required by AGENT.
- `types.rs`
  - shared domain and settlement state types.
- `client.rs`
  - local client state and transition log.
- `ledger.rs`
  - local BFT-domain ledger skeleton.
- `checkpoint.rs`
  - checkpoint and quorum structures + remote checkpoint verification helper.
- `causal.rs`
  - causality and lightcone helper logic.
- `crypto.rs`
  - crypto-era and renewal policy placeholders.
- `settlement.rs`
  - claim creation, verification, and status transition helpers.
- `sync.rs`
  - conflict detection, reconciliation, and resync hooks.
- `errors.rs`
  - protocol error vocabulary.
- `tests/`
  - scenario for future concrete tests.

## Build
- Placeholder-only implementation; replace each `ProtocolResult` path with real logic in phase order from `ROADMAP.md`.

