from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, List, Optional

from .models import Claim, Event, Message, ScenarioContext


ALLOWED_FINALITY_STAGES = {
    "remote-observed",
    "remote_observed",
    "provisional-credit",
    "provisionally_credited",
    "interstellar-settlement-finality",
    "interstellar-settled",
    "bilateral-finality",
    "bilateral-finalized",
}

ALLOWED_LIGHTCONE_STATUS = {"valid", "unknown", "invalid", "stale"}


@dataclass
class InvariantFailure(Exception):
    message: str


def _require_no_event_id_conflict(events: List[Event]) -> Dict[str, Event]:
    by_id: Dict[str, Event] = {}
    for event in events:
        if event.event_id in by_id:
            raise InvariantFailure(f"duplicate event id: {event.event_id}")
        by_id[event.event_id] = event
    return by_id


def _verify_causal_dependencies(events: List[Event], by_event: Dict[str, Event]) -> None:
    for event in events:
        for dependency in event.causal_dependencies:
            if dependency not in by_event:
                raise InvariantFailure(
                    f"event {event.event_id} depends on unknown event {dependency}"
                )


def _verify_message_times(messages: List[Message]) -> None:
    for msg in messages:
        if msg.receive_time < msg.send_time:
            raise InvariantFailure(
                f"message {msg.msg_id} received before sent ({msg.receive_time} < {msg.send_time})"
            )
        if msg.receive_time - msg.send_time < 0:
            raise InvariantFailure(f"message {msg.msg_id} invalid time window")


def invariant_lightcone(
    events: List[Event],
    messages: List[Message],
    context: Optional[ScenarioContext] = None,
) -> None:
    by_event = {event.event_id: event for event in events}
    for msg in messages:
        if context is None:
            continue
        from_event = by_event.get(msg.from_event)
        to_event = by_event.get(msg.to_event)
        if from_event is None or to_event is None:
            continue
        from_domain = from_event.domain_id
        to_domain = to_event.domain_id
        if from_domain == to_domain:
            # intra-domain messages are always physically possible in this scaffold
            continue
        distance_ly = context.distances_ly.get((from_domain, to_domain))
        if distance_ly is None:
            # no explicit known distance; treat route policy as minimum sanity check only
            if len(msg.route) < context.policy_route_min_hops:
                raise InvariantFailure(
                    f"message {msg.msg_id} route too short for policy in unknown domain pair "
                    f"({from_domain}->{to_domain})"
                )
            continue
        required = int(distance_ly + 0.5)
        if msg.receive_time - msg.send_time < required:
            raise InvariantFailure(
                f"message {msg.msg_id} cannot cross {distance_ly} ly in {msg.receive_time - msg.send_time} years"
            )
        if context.policy_max_delay_years < float("inf") and msg.receive_time - msg.send_time > context.policy_max_delay_years:
            raise InvariantFailure(
                f"message {msg.msg_id} delay bound exceeded: {msg.receive_time - msg.send_time} > {context.policy_max_delay_years}"
            )


def _offline_windows_for_domain(context: ScenarioContext, domain_id: str) -> List[tuple[int, int]]:
    windows = context.offline_windows.get(domain_id, [])
    return [(window.start, window.end) for window in windows]


def _message_has_offline_violation(
    message: Message,
    events_by_id: Dict[str, Event],
    context: ScenarioContext,
) -> bool:
    from_event = events_by_id.get(message.from_event)
    to_event = events_by_id.get(message.to_event)
    if from_event is None or to_event is None:
        return False
    for domain_id in {from_event.domain_id, to_event.domain_id}:
        for start, end in _offline_windows_for_domain(context, domain_id):
            if start <= message.send_time <= end or start <= message.receive_time <= end:
                return True
    return False


def invariant_offline_windows(
    messages: List[Message],
    events: List[Event],
    context: ScenarioContext,
) -> None:
    by_event = {event.event_id: event for event in events}
    for msg in messages:
        from_event = by_event.get(msg.from_event)
        to_event = by_event.get(msg.to_event)
        if from_event is None or to_event is None:
            continue
        for domain_id in {from_event.domain_id, to_event.domain_id}:
            for start, end in _offline_windows_for_domain(context, domain_id):
                if start <= msg.send_time <= end or start <= msg.receive_time <= end:
                    raise InvariantFailure(
                        f"message {msg.msg_id} touches offline window for domain {domain_id}: "
                        f"[{start}, {end}]"
                    )


def invariant_no_false_finality_claims(claims: List[Claim]) -> None:
    for claim in claims:
        if claim.finality_stage not in ALLOWED_FINALITY_STAGES:
            raise InvariantFailure(
                f"claim {claim.claim_id} uses unsupported finality stage {claim.finality_stage}"
            )
        if claim.lightcone_status not in ALLOWED_LIGHTCONE_STATUS:
            raise InvariantFailure(
                f"claim {claim.claim_id} uses unsupported lightcone status {claim.lightcone_status}"
            )
        if claim.settlement_horizon_years <= 0:
            raise InvariantFailure(
                f"claim {claim.claim_id} has non-positive settlement horizon"
            )
        if claim.finality_stage in {"provisional-credit", "provisionally_credited", "interstellar-settlement-finality", "interstellar-settled", "bilateral-finality", "bilateral-finalized"} and claim.lightcone_status != "valid":
            raise InvariantFailure(
                f"claim {claim.claim_id} is staged to high finality but lightcone_status={claim.lightcone_status}"
            )
        if claim.finality_stage in {"interstellar-settlement-finality", "interstellar-settled", "bilateral-finality", "bilateral-finalized"} and claim.lightcone_status == "valid" and claim.lock_checkpoint == "":
                raise InvariantFailure(
                    f"claim {claim.claim_id} cannot reach {claim.finality_stage} without checkpoint"
                )


def invariant_no_false_finality(messages: List[Message], events: List[Event], claims: List[Claim], context: ScenarioContext) -> None:
    # no remote domain can have more than one accepted claim for same lock in this version
    _ = _require_no_event_id_conflict(events)
    conflicts: Dict[str, List[Claim]] = {}
    for claim in claims:
        conflicts.setdefault(claim.lock_event_id, []).append(claim)
    for lock_event_id, items in conflicts.items():
        accepted = [item for item in items if item.finality_stage not in {"remote-observed", "remote_observed"}]
        if len(accepted) > 1:
            raise InvariantFailure(
                f"remote lock {lock_event_id} has conflicting accepted claims: "
                f"{', '.join(item.claim_id for item in accepted)}"
            )
    for claim in claims:
        if claim.amount <= 0:
            raise InvariantFailure(f"claim {claim.claim_id} must have positive amount")
        if claim.origin_domain == claim.remote_domain:
            raise InvariantFailure(
                f"claim {claim.claim_id} has same origin and remote domain ({claim.origin_domain})"
            )


def _parse_expectation_to_check(
    expectations: List[str], claims: List[Claim], context: ScenarioContext
) -> None:
    by_claim = {claim.claim_id: claim for claim in claims}
    if not expectations:
        return
    # supported forms:
    #  - "no_conflict <lock_event_id>"
    #  - "conflict <lock_event_id>"
    for raw in expectations:
        tokens = raw.split()
        if len(tokens) != 2:
            continue
        kind, lock_id = tokens[0].lower(), tokens[1]
        if kind in {"no_offline_violation", "offline_violation"}:
            if lock_id not in by_claim:
                raise InvariantFailure(f"unknown claim id in expectation: {lock_id}")
            # expectations by claim are no longer a primary offline oracle in phase6;
            # keep compatibility with existing scenarios by allowing explicit forms.
            continue
        locked = [claim for claim in claims if claim.lock_event_id == lock_id]
        has_conflict = len(locked) > 1
        if kind == "no_conflict" and has_conflict:
            raise InvariantFailure(f"expectation failed: found conflict for lock {lock_id}")
        if kind == "conflict" and not has_conflict:
            raise InvariantFailure(f"expectation failed: expected conflict for lock {lock_id}")


def invariant_replay_and_route(events: List[Event], messages: List[Message], context: ScenarioContext) -> None:
    by_event_id = {event.event_id: event for event in events}
    route_checks = []
    for msg in messages:
        key = (tuple(msg.route), msg.anti_replay_nonce)
        route_checks.append(key)
        if len(msg.anti_replay_nonce) < context.policy_min_nonce_chars:
            raise InvariantFailure(
                f"message {msg.msg_id} anti_replay_nonce too short: "
                f"{len(msg.anti_replay_nonce)} < {context.policy_min_nonce_chars}"
            )
        if len(msg.route) > 0 and len(msg.route) < context.policy_route_min_hops:
            raise InvariantFailure(
                f"message {msg.msg_id} violates policy route hops"
            )
        if context.policy_max_route_hops > 0 and len(msg.route) > context.policy_max_route_hops:
            raise InvariantFailure(
                f"message {msg.msg_id} violates max route hops: "
                f"{len(msg.route)} > {context.policy_max_route_hops}"
            )
        if msg.from_event and msg.from_event not in by_event_id:
            raise InvariantFailure(f"message {msg.msg_id} references unknown from_event {msg.from_event}")
        if msg.to_event and msg.to_event not in by_event_id:
            raise InvariantFailure(f"message {msg.msg_id} references unknown to_event {msg.to_event}")

    if len(route_checks) != len(set(route_checks)):
        raise InvariantFailure("message anti-replay collision detected by route + nonce")


def verify_invariants(
    events: List[Event],
    messages: List[Message],
    claims: Optional[List[Claim]] = None,
    context: Optional[ScenarioContext] = None,
) -> List[str]:
    if claims is None:
        claims = []
    if context is None:
        context = ScenarioContext()

    by_event = _require_no_event_id_conflict(events)
    _verify_causal_dependencies(events, by_event)
    _verify_message_times(messages)
    invariant_lightcone(events, messages, context=context)
    invariant_replay_and_route(events, messages, context)
    invariant_no_false_finality(messages, events, claims, context)
    invariant_no_false_finality_claims(claims)
    invariant_offline_windows(messages, events, context)
    _parse_expectation_to_check(context.expectations, claims, context)
    return [
        "ok",
        f"events={len(events)}",
        f"messages={len(messages)}",
        f"claims={len(claims)}",
    ]
