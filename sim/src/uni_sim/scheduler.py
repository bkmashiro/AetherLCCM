from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List, Tuple

from .models import Event, Message, ScenarioContext


@dataclass
class SimulationResult:
    events: List[Event]
    messages: List[Message]
    ordered_by_time: bool
    traces: List[str] = field(default_factory=list)
    dropped_replays: int = 0
    dropped_count: int = 0
    route_violations: int = 0
    delay_violations: int = 0


class SimulationScheduler:
    def __init__(self, context: ScenarioContext):
        self.context = context

    def run(self) -> SimulationResult:
        traces: List[str] = []
        events = sorted(
            self.context.events,
            key=lambda event: (event.local_sequence, event.coord.interval.t_min, event.event_id),
        )

        deduped_messages: List[Message] = []
        replay_index: Dict[Tuple[tuple[str, ...], str], Message] = {}

        messages = sorted(
            self.context.messages,
            key=lambda msg: (msg.send_time, msg.receive_time, msg.msg_id),
        )
        for msg in messages:
            route_tuple = tuple(msg.route)
            replay_key = (route_tuple, msg.anti_replay_nonce)
            previous = replay_index.get(replay_key)
            if previous is not None:
                traces.append(
                    f"drop replay msg={msg.msg_id} as duplicate of {previous.msg_id}"
                )
                continue
            replay_index[replay_key] = msg
            deduped_messages.append(msg)

        sorted_messages = sorted(
            deduped_messages,
            key=lambda msg: (msg.receive_time, msg.send_time, msg.msg_id),
        )
        ordered_by_time = all(
            earlier.receive_time <= later.receive_time
            for earlier, later in zip(sorted_messages, sorted_messages[1:])
        )

        dropped_replays = len(deduped_messages) != len(messages)
        if dropped_replays:
            traces.append(f"removed {len(messages) - len(deduped_messages)} replay duplicates")

        # route-aware deterministic pass
        max_route_hops = self.context.policy_max_route_hops
        max_delay = self.context.policy_max_delay_years
        route_violations = 0
        delay_violations = 0
        for message in sorted_messages:
            hop_count = max(0, len(message.route))
            if hop_count < self.context.policy_route_min_hops and message.route:
                traces.append(
                    f"route check: msg={message.msg_id} has {hop_count} hop(s), "
                    f"minimum required={self.context.policy_route_min_hops}"
                )
            if max_route_hops > 0 and hop_count > max_route_hops:
                route_violations += 1
                traces.append(
                    f"route bound violated: msg={message.msg_id} hops={hop_count} > max={max_route_hops}"
                )
            delay = message.receive_time - message.send_time
            if max_delay < float("inf") and delay > max_delay:
                delay_violations += 1
                traces.append(
                    f"delay bound violated: msg={message.msg_id} delay={delay} > max={max_delay}"
                )

        traces.append(f"scheduled {len(events)} events")
        traces.append(f"scheduled {len(sorted_messages)} messages")
        return SimulationResult(
            events=events,
            messages=sorted_messages,
            ordered_by_time=ordered_by_time,
            traces=traces,
            dropped_replays=dropped_replays,
            dropped_count=len(messages) - len(deduped_messages),
            route_violations=route_violations,
            delay_violations=delay_violations,
        )

    def export_trace(self, result: SimulationResult) -> dict:
        causal_edges: List[Dict[str, str]] = []
        event_order: Dict[str, int] = {event.event_id: index for index, event in enumerate(result.events)}
        for event in result.events:
            for dependency in event.causal_dependencies:
                causal_edges.append(
                    {
                        "from": dependency,
                        "to": event.event_id,
                        "kind": "event_dependency",
                    }
                )
        for message in result.messages:
            if message.from_event and message.to_event:
                causal_edges.append(
                    {
                        "from": message.from_event,
                        "to": message.to_event,
                        "kind": "message_delivery",
                        "message_id": message.msg_id,
                    }
                )

        return {
            "scenario_name": self.context.scenario_name,
            "policy": {
                "route_min_hops": self.context.policy_route_min_hops,
                "route_max_hops": self.context.policy_max_route_hops,
                "min_nonce_chars": self.context.policy_min_nonce_chars,
                "max_delay_years": self.context.policy_max_delay_years,
            },
            "event_count": len(result.events),
            "message_count": len(result.messages),
            "events": [
                {
                    "event_id": event.event_id,
                    "actor_id": event.actor_id,
                    "domain_id": event.domain_id,
                    "kind": str(event.kind),
                    "payload_hash": event.payload_hash,
                    "coord": {
                        "frame_id": event.coord.frame_id,
                        "interval": {
                            "t_min": event.coord.interval.t_min,
                            "t_max": event.coord.interval.t_max,
                        },
                        "region": {
                            "x": event.coord.region.x,
                            "y": event.coord.region.y,
                            "z": event.coord.region.z,
                        },
                        "uncertainty": event.coord.uncertainty,
                    },
                    "causal_dependencies": event.causal_dependencies,
                }
                for event in result.events
            ],
            "messages": [
                {
                    "msg_id": message.msg_id,
                    "from_event": message.from_event,
                    "to_event": message.to_event,
                    "route": message.route,
                    "send_time": message.send_time,
                    "receive_time": message.receive_time,
                    "anti_replay_nonce": message.anti_replay_nonce,
                }
                for message in result.messages
            ],
            "causal_graph": {
                "nodes": [event.event_id for event in result.events],
                "edges": causal_edges,
            },
            "ordering": {
                "event_index": {
                    event_id: index for event_id, index in event_order.items()
                },
                "trace_count": len(result.traces),
            },
            "traces": result.traces,
            "meta": {
                "ordered_by_time": result.ordered_by_time,
                "dropped_replays": result.dropped_replays,
                "dropped_count": result.dropped_count,
                "route_violations": result.route_violations,
                "delay_violations": result.delay_violations,
            },
        }

    def export_tla_trace(self, result: SimulationResult, claims=None) -> dict:
        if claims is None:
            claims = []
        states = []
        for step, event in enumerate(result.events):
            states.append(
                {
                    "index": step,
                    "type": "event",
                    "id": event.event_id,
                    "domain": event.domain_id,
                    "kind": str(event.kind),
                    "actor": event.actor_id,
                    "coord_t_min": event.coord.interval.t_min,
                    "coord_t_max": event.coord.interval.t_max,
                }
            )
        message_offset = len(states)
        for index, message in enumerate(result.messages):
            states.append(
                {
                    "index": message_offset + index,
                    "type": "message",
                    "id": message.msg_id,
                    "from_event": message.from_event,
                    "to_event": message.to_event,
                    "route": message.route,
                    "send_time": message.send_time,
                    "receive_time": message.receive_time,
                }
            )
        return {
            "scenario_name": self.context.scenario_name,
            "states": states,
            "invariants": {
                "ordered_by_time": result.ordered_by_time,
                "dropped_replays": result.dropped_replays,
                "route_violations": result.route_violations,
                "delay_violations": result.delay_violations,
            },
            "claims": [
                {
                    "claim_id": claim.claim_id,
                    "lock_event_id": claim.lock_event_id,
                    "origin_domain": claim.origin_domain,
                    "remote_domain": claim.remote_domain,
                    "amount": claim.amount,
                    "finality_stage": claim.finality_stage,
                    "lightcone_status": claim.lightcone_status,
                }
                for claim in claims
            ],
            "variables": {
                "event_count": len(result.events),
                "message_count": len(result.messages),
                "claim_count": len(claims),
            },
        }

    def export_alloy_graph(self, result: SimulationResult) -> dict:
        # deterministic, Alloy-like JSON shape that can be translated to .als predicates.
        domain_edges = set()
        for message in result.messages:
            if message.from_event and message.to_event:
                from_event = None
                to_event = None
                for event in result.events:
                    if event.event_id == message.from_event:
                        from_event = event
                    if event.event_id == message.to_event:
                        to_event = event
                if from_event is None or to_event is None:
                    continue
                domain_edges.add((from_event.domain_id, to_event.domain_id))

        return {
            "signature": {
                "Event": [event.event_id for event in result.events],
                "Message": [msg.msg_id for msg in result.messages],
            },
            "edge": [
                {
                    "source": source,
                    "target": target,
                    "kind": "inter_domain_message",
                }
                for source, target in sorted(domain_edges)
            ],
            "domain": sorted(list({event.domain_id for event in result.events})),
        }
