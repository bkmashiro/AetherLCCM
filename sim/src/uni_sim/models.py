from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List, Tuple


@dataclass
class Position3D:
    x: float
    y: float
    z: float


@dataclass
class TimeInterval:
    t_min: int
    t_max: int


@dataclass
class SpacetimeCoord:
    frame_id: str
    interval: TimeInterval
    region: Position3D
    uncertainty: float = 0.0
    attestation: List[str] = field(default_factory=list)


@dataclass
class Event:
    event_id: str
    actor_id: str
    domain_id: str
    kind: "EventKind"
    payload_hash: str
    coord: SpacetimeCoord
    local_sequence: int = 0
    causal_dependencies: List[str] = field(default_factory=list)
    signatures: List[str] = field(default_factory=list)


@dataclass
class Message:
    msg_id: str
    from_event: str
    to_event: str
    payload_hash: str
    route: List[str]
    anti_replay_nonce: str
    send_time: int
    receive_time: int
    observed_by: List[str] = field(default_factory=list)


@dataclass
class Checkpoint:
    checkpoint_id: str
    domain_id: str
    height: int
    hash: str


@dataclass
class Claim:
    claim_id: str
    lock_event_id: str
    origin_domain: str
    remote_domain: str
    asset_id: str
    amount: int
    settlement_horizon_years: int
    lock_checkpoint: str
    lightcone_status: str = "unknown"
    finality_stage: str = "remote_observed"


@dataclass
class OfflineWindow:
    domain_id: str
    start: int
    end: int

    def contains(self, timestamp: int) -> bool:
        return self.start <= timestamp <= self.end


@dataclass
class Worldline:
    name: str
    coord_by_time: Dict[int, SpacetimeCoord] = field(default_factory=dict)

    def location_at(self, t: int) -> SpacetimeCoord:
        if t in self.coord_by_time:
            return self.coord_by_time[t]
        if not self.coord_by_time:
            raise KeyError("worldline has no coordinates")
        last_time = max(self.coord_by_time.keys())
        if t < last_time:
            return self.coord_by_time[last_time]
        return self.coord_by_time[last_time]


@dataclass
class Domain:
    domain_id: str
    name: str
    trust_root: str


@dataclass
class ScenarioContext:
    scenario_name: str = ""
    domains: Dict[str, Domain] = field(default_factory=dict)
    worldlines: Dict[str, Worldline] = field(default_factory=dict)
    messages: List[Message] = field(default_factory=list)
    events: List[Event] = field(default_factory=list)
    claims: List[Claim] = field(default_factory=list)
    checkpoints: List[Checkpoint] = field(default_factory=list)
    expectations: List[str] = field(default_factory=list)
    distances_ly: Dict[Tuple[str, str], float] = field(default_factory=dict)
    relay_default_delay_years: float = 0.0
    policy_route_min_hops: int = 1
    policy_min_nonce_chars: int = 8
    policy_max_route_hops: int = 0
    policy_max_delay_years: float = float("inf")
    offline_windows: Dict[str, List[OfflineWindow]] = field(default_factory=dict)
    # optional human-readable output from parser and scheduler
    parse_warnings: List[str] = field(default_factory=list)


@dataclass(frozen=True)
class EventKind:
    value: str

    def __str__(self) -> str:
        return self.value


def event_kind(val: str) -> EventKind:
    return EventKind(val)
