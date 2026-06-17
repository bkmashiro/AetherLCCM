from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

import re

from .models import (
    Claim,
    Checkpoint,
    Domain,
    Event,
    EventKind,
    Message,
    OfflineWindow,
    Position3D,
    ScenarioContext,
    SpacetimeCoord,
    TimeInterval,
    Worldline,
)


_ASSIGNMENT = re.compile(r"^([A-Za-z_][A-Za-z0-9_-]*)=(.+)$")


def _split_tokens(line: str) -> List[str]:
    return [tok for tok in re.findall(r"\"[^\"]+\"|\S+", line) if tok]


def _parse_assignments(parts: List[str]) -> tuple[Dict[str, str], List[str]]:
    assignments: Dict[str, str] = {}
    positional: List[str] = []
    for part in parts:
        match = _ASSIGNMENT.match(part)
        if match:
            assignments[match.group(1)] = match.group(2).strip().strip('"')
        else:
            positional.append(part)
    return assignments, positional


def _parse_float(value: str, *, default: float = 0.0) -> float:
    try:
        return float(value.replace("ly", "").replace("years", "").strip())
    except ValueError:
        return default


def _parse_int(value: str, *, default: int = 0) -> int:
    try:
        return int(value)
    except ValueError:
        return default


def _parse_route(raw: str) -> List[str]:
    if not raw:
        return []
    cleaned = raw.replace("->", ">")
    if " " in cleaned and ">" not in cleaned and "," not in cleaned:
        return [segment for segment in cleaned.split() if segment]
    if ">" in cleaned:
        return [segment.strip() for segment in cleaned.split(">") if segment.strip()]
    if "," in cleaned:
        return [segment.strip() for segment in cleaned.split(",") if segment.strip()]
    return [cleaned.strip()]


def _to_domain(name: str) -> str:
    return name.strip().lower()


def _coord_from_kv(
    domain_id: str,
    kv: Dict[str, str],
    frame_by_domain: Dict[str, str],
) -> SpacetimeCoord:
    frame_id = frame_by_domain.get(domain_id, domain_id)
    x = float(kv.get("x", "0") or 0)
    y = float(kv.get("y", "0") or 0)
    z = float(kv.get("z", "0") or 0)
    uncertainty = float(kv.get("uncertainty", "0") or 0)
    t_min = _parse_int(kv.get("t", kv.get("t_min", "0")), default=0)
    t_max = _parse_int(kv.get("t_max", str(t_min + 1)), default=t_min + 1)
    return SpacetimeCoord(
        frame_id=frame_id,
        interval=TimeInterval(t_min=t_min, t_max=max(t_min, t_max)),
        region=Position3D(x=x, y=y, z=z),
        uncertainty=uncertainty,
    )


@dataclass
class Scenario:
    name: str
    source: str
    context: ScenarioContext = field(default_factory=ScenarioContext)
    raw_blocks: List[str] = field(default_factory=list)


@dataclass
class ParseResult:
    scenario: Scenario
    warnings: List[str]


class ScenarioParser:
    @staticmethod
    def parse(text: str, name: str = "unnamed") -> ParseResult:
        lines = text.splitlines()
        warnings: List[str] = []
        context = ScenarioContext(scenario_name=name)
        context.scenario_name = name

        first_domain: Optional[str] = None
        frame_by_domain: Dict[str, str] = {}

        for line_num, raw_line in enumerate(lines, start=1):
            line = raw_line.strip()
            if not line or line.startswith("#") or line in {"{", "}"}:
                continue
            if line.startswith("scenario "):
                header = line.split("{", 1)[0]
                parts = header.split()
                if len(parts) >= 2:
                    name = parts[1]
                    context.scenario_name = name
                else:
                    warnings.append(f"line {line_num}: empty scenario declaration")
                continue

            tokens = _split_tokens(line)
            if not tokens:
                continue
            command, raw_args = tokens[0].lower(), tokens[1:]
            kv, positional = _parse_assignments(raw_args)

            if command == "domain":
                if len(positional) < 1:
                    warnings.append(f"line {line_num}: domain command missing identifier")
                    continue
                domain_id = _to_domain(positional[0])
                frame_id = kv.get("frame")
                if frame_id is None and len(positional) >= 3 and positional[1].lower() == "at" and positional[2].lower() == "frame":
                    if len(positional) >= 4:
                        frame_id = positional[3]
                if frame_id is None:
                    frame_id = domain_id
                context.domains[domain_id] = Domain(
                    domain_id=domain_id,
                    name=domain_id,
                    trust_root=kv.get("trust_root", ""),
                )
                frame_by_domain[domain_id] = frame_id
                if first_domain is None:
                    first_domain = domain_id
                continue

            if command == "distance":
                if len(positional) < 3:
                    warnings.append(f"line {line_num}: distance requires two domains and one value")
                    continue
                d0 = _to_domain(positional[0])
                d1 = _to_domain(positional[1])
                distance_ly = _parse_float(positional[2], default=0.0)
                context.distances_ly[(d0, d1)] = distance_ly
                context.distances_ly[(d1, d0)] = distance_ly
                continue

            if command == "worldline":
                if not positional:
                    warnings.append(f"line {line_num}: worldline missing identifier")
                    continue
                worldline_id = positional[0]
                domain_id = _to_domain(kv.get("domain", first_domain or worldline_id))
                t = _parse_int(kv.get("t", "0"), default=0)
                x = float(kv.get("x", "0") or 0)
                y = float(kv.get("y", "0") or 0)
                z = float(kv.get("z", "0") or 0)
                interval = TimeInterval(t_min=t, t_max=max(t, t + 1))
                coord = SpacetimeCoord(
                    frame_id=frame_by_domain.get(domain_id, domain_id),
                    interval=interval,
                    region=Position3D(x=x, y=y, z=z),
                )
                wl = context.worldlines.setdefault(worldline_id, Worldline(name=worldline_id))
                wl.coord_by_time[t] = coord
                continue

            if command == "event":
                if len(positional) < 2:
                    warnings.append(f"line {line_num}: event command requires kind and id")
                    continue
                kind = positional[0]
                event_id = positional[1]
                actor_id = kv.get("actor", kv.get("by", f"actor-{event_id}"))
                domain_id = _to_domain(kv.get("domain", first_domain or "unknown"))
                coord = _coord_from_kv(
                    domain_id=domain_id,
                    kv=kv,
                    frame_by_domain=frame_by_domain,
                )
                payload_hash = kv.get("hash", kv.get("payload_hash", f"{kind}:{event_id}:{actor_id}"))
                local_sequence = _parse_int(kv.get("local_sequence", kv.get("seq", "0")), default=0)
                deps_raw = kv.get("deps", "")
                dependencies = [dep.strip() for dep in deps_raw.split(",") if dep.strip()]
                signatures = [sig.strip() for sig in kv.get("signatures", "").split(",") if sig.strip()]
                context.events.append(
                    Event(
                        event_id=event_id,
                        actor_id=actor_id,
                        domain_id=domain_id,
                        kind=EventKind(kind),
                        payload_hash=payload_hash,
                        coord=coord,
                        local_sequence=local_sequence,
                        causal_dependencies=dependencies,
                        signatures=signatures,
                    )
                )
                continue

            if command == "message":
                if len(positional) < 1:
                    warnings.append(f"line {line_num}: message command missing id")
                    continue
                msg_id = positional[0]
                from_event = kv.get("from", kv.get("from_event", ""))
                to_event = kv.get("to", kv.get("to_event", ""))
                route = _parse_route(kv.get("route", ""))
                anti_replay_nonce = kv.get("anti_replay_nonce", kv.get("nonce", msg_id))
                send_time = _parse_int(kv.get("send", kv.get("send_time", "0")), default=0)
                receive_time = _parse_int(
                    kv.get("receive", kv.get("receive_time", str(send_time))),
                    default=send_time,
                )
                observed_by = [value.strip() for value in kv.get("observed_by", "").split(",") if value.strip()]
                payload_hash = kv.get("payload_hash", f"{from_event}->{to_event}:{msg_id}")
                context.messages.append(
                    Message(
                        msg_id=msg_id,
                        from_event=from_event,
                        to_event=to_event,
                        payload_hash=payload_hash,
                        route=route,
                        anti_replay_nonce=anti_replay_nonce,
                        send_time=send_time,
                        receive_time=receive_time,
                        observed_by=observed_by,
                    )
                )
                continue

            if command == "checkpoint":
                if len(positional) < 1:
                    warnings.append(f"line {line_num}: checkpoint command missing id")
                    continue
                checkpoint_id = positional[0]
                domain_id = _to_domain(kv.get("domain", first_domain or "unknown"))
                height = _parse_int(kv.get("height", "1"), default=1)
                hash_value = kv.get("hash", kv.get("multi_hash", f"hash-{checkpoint_id}"))
                context.checkpoints.append(
                    Checkpoint(
                        checkpoint_id=checkpoint_id,
                        domain_id=domain_id,
                        height=height,
                        hash=hash_value,
                    )
                )
                continue

            if command == "claim":
                if not positional:
                    warnings.append(f"line {line_num}: claim command missing id")
                    continue
                claim_id = positional[0]
                missing = [field for field in ("lock", "origin", "remote", "asset", "amount", "checkpoint") if field not in kv]
                if missing:
                    warnings.append(f"line {line_num}: claim missing fields {', '.join(missing)}")
                    continue
                context.claims.append(
                    Claim(
                        claim_id=claim_id,
                        lock_event_id=kv["lock"],
                        origin_domain=_to_domain(kv["origin"]),
                        remote_domain=_to_domain(kv["remote"]),
                        asset_id=kv["asset"],
                        amount=_parse_int(kv["amount"], default=0),
                        settlement_horizon_years=_parse_int(kv.get("horizon", kv.get("settlement_horizon", "0")), default=0),
                        lock_checkpoint=kv["checkpoint"],
                        lightcone_status=kv.get("lightcone_status", "unknown"),
                        finality_stage=kv.get("finality_stage", "remote-observed"),
                    )
                )
                continue

            if command == "policy":
                route_min_hops = kv.get("route_min_hops")
                if route_min_hops is not None:
                    context.policy_route_min_hops = max(1, _parse_int(route_min_hops, default=1))
                min_nonce_chars = kv.get("min_nonce_chars")
                if min_nonce_chars is not None:
                    context.policy_min_nonce_chars = max(4, _parse_int(min_nonce_chars, default=8))
                max_route_hops = kv.get("max_route_hops")
                if max_route_hops is not None:
                    parsed = _parse_int(max_route_hops, default=0)
                    context.policy_max_route_hops = max(0, parsed)
                max_delay_years = kv.get("max_delay_years")
                if max_delay_years is not None:
                    context.policy_max_delay_years = _parse_float(max_delay_years, default=float("inf"))
                    if context.policy_max_delay_years <= 0:
                        context.policy_max_delay_years = float("inf")
                continue

            if command == "relay":
                delay = kv.get("delay")
                if delay is not None:
                    context.relay_default_delay_years = _parse_float(delay, default=0.0)
                continue

            if command == "offline":
                domain_id = _to_domain(kv.get("domain", kv.get("at", first_domain or "")))
                start = _parse_int(kv.get("start", "0"), default=0)
                end = _parse_int(kv.get("end", str(start)), default=start)
                if not domain_id:
                    warnings.append(f"line {line_num}: offline missing domain")
                    continue
                if end < start:
                    start, end = end, start
                context.offline_windows.setdefault(domain_id, []).append(OfflineWindow(domain_id=domain_id, start=start, end=end))
                continue

            if command == "expect":
                raw = " ".join(positional)
                if raw:
                    context.expectations.append(" ".join(raw.split()))
                continue

            warnings.append(f"line {line_num}: unknown command '{command}'")

        if not context.domains:
            warnings.append("no domain declarations found in scenario")
        raw_blocks = [line.strip() for line in lines if line.strip()]
        if not raw_blocks:
            warnings.append("empty scenario source")

        return ParseResult(
            scenario=Scenario(name=name, source=text, context=context, raw_blocks=raw_blocks),
            warnings=warnings,
        )

    @staticmethod
    def parse_file(path: str) -> ParseResult:
        with open(path, "r", encoding="utf-8") as f:
            return ScenarioParser.parse(f.read(), name=path)
