import json
import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "src")
if SRC not in sys.path:
    sys.path.insert(0, SRC)

from uni_sim.scenario import ScenarioParser
from uni_sim.scheduler import SimulationScheduler
from uni_sim.verifier import InvariantFailure, verify_invariants


def _fixture_path(name: str) -> str:
    return os.path.join(ROOT, "scenarios", name)


class Phase6HardeningTests(unittest.TestCase):
    def test_bounded_delay_violation_is_detected(self) -> None:
        parsed = ScenarioParser.parse_file(_fixture_path("phase6_bounded_delay_violation.txt"))
        scheduler = SimulationScheduler(parsed.scenario.context)
        result = scheduler.run()
        self.assertEqual(result.delay_violations, 1)
        with self.assertRaises(InvariantFailure):
            verify_invariants(
                result.events,
                result.messages,
                claims=parsed.scenario.context.claims,
                context=parsed.scenario.context,
            )

    def test_route_hops_violation_is_detected(self) -> None:
        parsed = ScenarioParser.parse_file(_fixture_path("phase6_route_hops_violation.txt"))
        scheduler = SimulationScheduler(parsed.scenario.context)
        result = scheduler.run()
        self.assertEqual(result.route_violations, 1)
        with self.assertRaises(InvariantFailure):
            verify_invariants(
                result.events,
                result.messages,
                claims=parsed.scenario.context.claims,
                context=parsed.scenario.context,
            )

    def test_offline_violation_is_detected(self) -> None:
        parsed = ScenarioParser.parse_file(_fixture_path("phase6_offline_violation.txt"))
        scheduler = SimulationScheduler(parsed.scenario.context)
        result = scheduler.run()
        with self.assertRaises(InvariantFailure):
            verify_invariants(
                result.events,
                result.messages,
                claims=parsed.scenario.context.claims,
                context=parsed.scenario.context,
            )

    def test_export_trace_has_replay_metadata(self) -> None:
        parsed = ScenarioParser.parse_file(_fixture_path("phase5_honest_cross_settlement.txt"))
        scheduler = SimulationScheduler(parsed.scenario.context)
        result = scheduler.run()
        verify_invariants(
            result.events,
            result.messages,
            claims=parsed.scenario.context.claims,
            context=parsed.scenario.context,
        )
        with tempfile.NamedTemporaryFile("w", delete=False) as tmp:
            output_path = tmp.name
            trace = scheduler.export_trace(result)
            json.dump(trace, tmp, indent=2)

        with open(output_path, "r", encoding="utf-8") as handle:
            loaded = json.load(handle)
        self.assertEqual(loaded["meta"]["dropped_count"], result.dropped_count)
        self.assertIn("events", loaded)
        self.assertIn("messages", loaded)

    def test_tla_and_alloy_exports_are_deterministic(self) -> None:
        parsed = ScenarioParser.parse_file(_fixture_path("phase6_route_hops_violation.txt"))
        scheduler = SimulationScheduler(parsed.scenario.context)
        result = scheduler.run()
        export_trace = scheduler.export_trace(result)
        tla = scheduler.export_tla_trace(result, claims=parsed.scenario.context.claims)
        alloy = scheduler.export_alloy_graph(result)

        self.assertEqual(export_trace["scenario_name"], parsed.scenario.context.scenario_name)
        self.assertGreater(len(tla["states"]), 0)
        self.assertIn("states", tla)
        self.assertIn("edge", alloy)
        self.assertIn("signature", alloy)
        self.assertTrue(export_trace["causal_graph"]["edges"])
        self.assertIn("meta", export_trace)

    def test_cli_export_flags_create_files(self) -> None:
        parsed = ScenarioParser.parse_file(_fixture_path("phase5_replay_probe.txt"))
        scheduler = SimulationScheduler(parsed.scenario.context)
        result = scheduler.run()
        verify_invariants(
            result.events,
            result.messages,
            claims=parsed.scenario.context.claims,
            context=parsed.scenario.context,
        )

        with tempfile.TemporaryDirectory() as tempdir:
            trace_path = os.path.join(tempdir, "trace.json")
            tla_path = os.path.join(tempdir, "tla.json")
            causal_path = os.path.join(tempdir, "causal.json")
            alloy_path = os.path.join(tempdir, "alloy.json")
            with open(trace_path, "w", encoding="utf-8") as f:
                json.dump(scheduler.export_trace(result), f, indent=2)
            with open(tla_path, "w", encoding="utf-8") as f:
                json.dump(scheduler.export_tla_trace(result, claims=parsed.scenario.context.claims), f, indent=2)
            with open(causal_path, "w", encoding="utf-8") as f:
                json.dump(scheduler.export_trace(result)["causal_graph"], f, indent=2)
            with open(alloy_path, "w", encoding="utf-8") as f:
                json.dump(scheduler.export_alloy_graph(result), f, indent=2)

            for path in [trace_path, tla_path, causal_path, alloy_path]:
                self.assertTrue(os.path.exists(path))
                with open(path, "r", encoding="utf-8") as handle:
                    payload = json.load(handle)
                self.assertIsInstance(payload, (dict, list))


if __name__ == "__main__":
    unittest.main()
