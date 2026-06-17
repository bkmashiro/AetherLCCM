# Universe-Scale Exchange Protocol Scaffold

This repository contains a blueprint and baseline code structure for the LCCM protocol.

## Key files

- [AGENT.MD](/C:/Users/Dylan/Desktop/uni/AGENT.MD)
  - Implementation contract and governing constraints.
- [ROADMAP.md](/C:/Users/Dylan/Desktop/uni/ROADMAP.md)
  - Phase plan and acceptance criteria.
- `protocol/` 
  - Rust crate scaffold for consensus, checkpoints, causality, and settlement transition APIs.
- `sim/`
  - Python scenario + simulation + invariant scaffolding.

## Implemented scaffold outputs

- Canonical protocol APIs (placeholder-only implementations):
  - `submit_local_tx`
  - `lock_for_export`
  - `create_settlement_claim`
  - `verify_remote_checkpoint`
  - `verify_settlement_claim`
  - `accept_remote_claim`
  - `provisionally_credit`
  - `reconcile_checkpoint`
  - `detect_conflict`
  - `slash_or_dispute`
  - `resync_after_offline`

## Next recommended steps

1. Replace placeholder errors with executable protocol logic.
2. Wire deterministic trace output into Rust/Python boundaries.
3. Expand Python scenario parser and property checks to cover the J-section scenarios in `uni.md`.

