import os
import sys
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


class ScenarioDslTests(unittest.TestCase):
    def test_parse_honest_cross_settlement(self) -> None:
        parsed = ScenarioParser.parse_file(_fixture_path("phase5_honest_cross_settlement.txt"))
        context = parsed.scenario.context
        self.assertIn("earth", context.domains)
        self.assertIn("alpha", context.domains)
        self.assertGreater(context.distances_ly.get(("earth", "alpha"), 0), 0.0)
        self.assertEqual(len(context.claims), 1)
        self.assertEqual(context.claims[0].claim_id, "claim-001")


class SchedulerTests(unittest.TestCase):
    def test_scheduler_determinism_and_replay_drop(self) -> None:
        parsed = ScenarioParser.parse_file(_fixture_path("phase5_replay_probe.txt"))
        scheduler = SimulationScheduler(parsed.scenario.context)
        result = scheduler.run()
        self.assertTrue(result.ordered_by_time)
        self.assertEqual(result.dropped_count, 1)
        self.assertEqual(len(result.messages), 1)


class InvariantTests(unittest.TestCase):
    def test_honest_scenario_invariants(self) -> None:
        parsed = ScenarioParser.parse_file(_fixture_path("phase5_honest_cross_settlement.txt"))
        scheduler = SimulationScheduler(parsed.scenario.context)
        result = scheduler.run()
        output = verify_invariants(
            result.events,
            result.messages,
            claims=parsed.scenario.context.claims,
            context=parsed.scenario.context,
        )
        self.assertIn("ok", output)
        self.assertIn("events=2", output[1])

    def test_double_lock_conflict_is_detected(self) -> None:
        parsed = ScenarioParser.parse_file(_fixture_path("phase5_double_lock_conflict.txt"))
        scheduler = SimulationScheduler(parsed.scenario.context)
        result = scheduler.run()
        with self.assertRaises(InvariantFailure):
            verify_invariants(
                result.events,
                result.messages,
                claims=parsed.scenario.context.claims,
                context=parsed.scenario.context,
            )


if __name__ == "__main__":
    unittest.main()
