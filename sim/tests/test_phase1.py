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
from uni_sim.models import Message, ScenarioContext


class Phase1ScenarioTests(unittest.TestCase):
    def test_empty_scenario_warning(self) -> None:
        result = ScenarioParser.parse("")
        self.assertTrue(any("empty scenario source" in warning for warning in result.warnings))

    def test_scheduler_orders_messages(self) -> None:
        context = ScenarioContext()
        context.messages.append(
            Message(
                msg_id="m1",
                from_event="e1",
                to_event="e2",
                payload_hash="p1",
                route=[],
                anti_replay_nonce="n1",
                send_time=10,
                receive_time=20,
            )
        )
        context.messages.append(
            Message(
                msg_id="m0",
                from_event="e0",
                to_event="e1",
                payload_hash="p0",
                route=[],
                anti_replay_nonce="n0",
                send_time=5,
                receive_time=5,
            )
        )
        scheduler = SimulationScheduler(context)
        result = scheduler.run()
        self.assertEqual(result.messages[0].msg_id, "m0")
        self.assertEqual(result.messages[1].msg_id, "m1")

    def test_lightcone_invariant_checks_receive_after_send(self) -> None:
        messages = [
            Message(
                msg_id="bad",
                from_event="e1",
                to_event="e2",
                payload_hash="bad",
                route=[],
                anti_replay_nonce="n",
                send_time=10,
                receive_time=9,
            )
        ]
        with self.assertRaises(InvariantFailure):
            verify_invariants([], messages)


if __name__ == "__main__":
    unittest.main()

