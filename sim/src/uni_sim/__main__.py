import argparse
import json

from .scenario import ScenarioParser
from .scheduler import SimulationScheduler
from .verifier import verify_invariants


def main() -> None:
    parser = argparse.ArgumentParser(description="Run Universe exchange simulation scenarios")
    parser.add_argument("scenario_file", nargs="?", default=None, help="Path to a .md/.txt scenario file")
    parser.add_argument("--text", default=None, help="Inline scenario text")
    parser.add_argument("--trace-json", default=None, help="Optional deterministic trace output path")
    parser.add_argument("--tla-json", default=None, help="Optional TLA+ style trace output path")
    parser.add_argument("--causal-json", default=None, help="Optional causal graph output path")
    parser.add_argument("--alloy-json", default=None, help="Optional Alloy-style dependency output path")
    args = parser.parse_args()

    if args.text is not None:
        parsed = ScenarioParser.parse(args.text, name="inline")
    elif args.scenario_file is not None:
        parsed = ScenarioParser.parse_file(args.scenario_file)
    else:
        parsed = ScenarioParser.parse("scenario empty {}", name="empty")
    parse_warnings = parsed.warnings

    scheduler = SimulationScheduler(parsed.scenario.context)
    result = scheduler.run()
    verify_invariants(
        result.events,
        result.messages,
        claims=parsed.scenario.context.claims,
        context=parsed.scenario.context,
    )
    if args.trace_json:
        with open(args.trace_json, "w", encoding="utf-8") as f:
            json.dump(scheduler.export_trace(result), f, indent=2)
    if args.tla_json:
        with open(args.tla_json, "w", encoding="utf-8") as f:
            json.dump(
                scheduler.export_tla_trace(result, claims=parsed.scenario.context.claims),
                f,
                indent=2,
            )
    if args.causal_json:
        trace = scheduler.export_trace(result)
        with open(args.causal_json, "w", encoding="utf-8") as f:
            json.dump(trace["causal_graph"], f, indent=2)
    if args.alloy_json:
        with open(args.alloy_json, "w", encoding="utf-8") as f:
            json.dump(scheduler.export_alloy_graph(result), f, indent=2)
    print(f"scenario={parsed.scenario.name} events={len(result.events)} messages={len(result.messages)}")
    print(f"ordered_by_time={result.ordered_by_time} dropped_replays={result.dropped_count}")
    for warning in parse_warnings:
        print(f"warning: {warning}")
    for trace in result.traces:
        print(trace)


if __name__ == "__main__":
    main()
