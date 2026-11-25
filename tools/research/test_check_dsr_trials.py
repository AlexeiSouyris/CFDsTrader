"""Tests of check_dsr_trials.py (#660). Run: python3 -m unittest discover -s tools/research."""

from __future__ import annotations

import unittest

from check_dsr_trials import N_BASE, check, count_trials, main


def row(id_: str, programa: str, veredicto: str = "no pasa") -> dict[str, str]:
    return {"id": id_, "programa": programa, "veredicto": veredicto}


ROWS = [
    row("R001", "estrategia en uso", "pierde"),
    row("R010", "ML", "no mejora"),
    row("R014", "programa 1 ronda 1"),
    row("R021", "programa 2 ronda 3 (descriptivo)", "descriptivo"),
    row("R022", "descriptivo (revisión externa)", "descriptivo"),
    row("R034", "programa 5 H5.1", "no pasa (corrida inválida; reemplazada por R035)"),
    row("R035", "programa 5 H5.1"),
]


class CountTest(unittest.TestCase):
    def test_counts_ml_and_voting_hypotheses_only(self) -> None:
        count = count_trials(ROWS)
        self.assertEqual(count.ml, ["R010"])
        self.assertEqual(count.hypotheses, ["R014", "R035"])
        self.assertEqual(count.replaced, ["R034"])
        self.assertEqual(count.expected, N_BASE + 3)


class CheckTest(unittest.TestCase):
    def test_equal_passes(self) -> None:
        ok, _ = check({"rule": {"dsr_trials": N_BASE + 3}}, ROWS)
        self.assertTrue(ok)

    def test_frozen_but_not_run_passes_with_notice(self) -> None:
        ok, lines = check({"rule": {"dsr_trials": N_BASE + 5}}, ROWS)
        self.assertTrue(ok)
        self.assertTrue(any(line.startswith("aviso") for line in lines))

    def test_uncounted_test_fails(self) -> None:
        ok, _ = check({"rule": {"dsr_trials": N_BASE + 2}}, ROWS)
        self.assertFalse(ok)

    def test_decreasing_value_fails(self) -> None:
        config = {"a": {"dsr_trials": N_BASE + 4}, "b": {"dsr_trials": N_BASE + 3}}
        ok, _ = check(config, ROWS)
        self.assertFalse(ok)

    def test_no_dsr_trials_fails(self) -> None:
        ok, _ = check({"orb": {"range_candles": 2}}, ROWS)
        self.assertFalse(ok)

    def test_repository_files_reconcile(self) -> None:
        self.assertEqual(main([]), 0)


if __name__ == "__main__":
    unittest.main()
