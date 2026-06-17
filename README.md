# AetherLCCM Exchange

## Overview

AetherLCCM is a research scaffold for a distributed, universe-scale exchange system based on **LCCM**:

- **L**ocal **C**onsensus **C**heckpointing
- **C**ausal checkpoint dependency graph
- **M**ultiple risk-aware settlement stages for inter-domain credit

The design rejects global immediate finality across space and instead uses deterministic local finality with explicit, policy-governed credit upgrade paths.

## Design intent

The project separates concerns into three layers:

- **Local Ledger Domain**: each domain finalizes events via BFT-style local checkpoints.
- **Causal Checkpoint Mesh**: cross-domain checkpoints are relayed, observed, and validated by causal dependency + route + freshness constraints.
- **Clearing and Credit Overlay**: claims are introduced conservatively (`remote-observed`), then become provisionally credited, then bilaterally settled under challenge.

This structure is intended to model physically-plausible settlement where information delay and trust boundaries are explicit.

## Repository structure

- `protocol/`  
  Rust crate with core types and transition APIs for checkpoints, settlement claims, causality checks, disputes, and sync paths.
- `sim/`  
  Python scenario engine that parses event traces, schedules delivery, and verifies invariants for safety, light-cone validity, anti-replay, and offline behavior.
- `scripts/`  
  Small helper scripts for running phase checks locally.
- `docs/`  
  Additional notes and support material.

## Implemented protocol API surface

The following protocol methods are the canonical interfaces used by the simulator and roadmap work:

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

## Core guarantees this scaffold enforces

- Local finality is immutable for finalized local checkpoints.
- Non-local claims are risk-tagged; no automatic global-final behavior.
- Light-cone validity gates accepted inter-domain messages.
- Replay and stale-proof protections are first-class.
- Conflicting inter-domain claims are detectable and routed to dispute/sync paths.

## Quick start

From the repo root:

1. Set up Rust and Python environments.
2. Run Python scenario tests in `sim/`.
3. Run Rust unit tests in `protocol/`.

Each module includes its own README for local execution details and assumptions.

## Typical development flow

1. Add/edit a scenario in `sim/scenarios`.
2. Extend invariant checks in `sim/src/uni_sim`.
3. Expand protocol behavior in `protocol/src`.
4. Re-run simulations/tests for the targeted phase.

## Notes

This repository is intentionally structured as a staged, auditable implementation path rather than a complete production system.
