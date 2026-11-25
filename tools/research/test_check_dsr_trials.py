"""Tests of check_dsr_trials.py (#660, #690). Run: python3 -m unittest discover -s tools/research."""

from __future__ import annotations

import unittest
from datetime import date
from decimal import Decimal

from check_dsr_trials import (
    LEDGER_END,
    LEDGER_START,
    check,
    check_ledger,
    count_trials,
    load_budget,
    main,
    parse_ledger,
    parse_percent,
)

BASE = 344
BUDGET_SECTION = {
    "alpha": "0.05",
    "initial_wealth": "0.0475",
    "start": "2026-11-25",
    "max_bet": "0.0125",
    "payout": "0.05",
    "edge_cost_factor": "2",
    "default_ic": "0.05",
    "dsr_trials_base": BASE,
}
BUDGET = load_budget({"alpha_budget": BUDGET_SECTION})


def config(**sections: dict[str, object]) -> dict[str, object]:
    return {**sections, "alpha_budget": BUDGET_SECTION}


def row(
    id_: str, programa: str, veredicto: str = "no pasa", fecha: str = "2026-11-24"
) -> dict[str, str]:
    return {"id": id_, "fecha": fecha, "programa": programa, "veredicto": veredicto}


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
        count = count_trials(ROWS, BASE)
        self.assertEqual(count.ml, ["R010"])
        self.assertEqual(count.hypotheses, ["R014", "R035"])
        self.assertEqual(count.replaced, ["R034"])
        self.assertEqual(count.strategy, [])
        self.assertEqual(count.expected, BASE + 3)

    def test_strategy_rows_count_from_the_epoch_start(self) -> None:
        rows = [*ROWS, row("R099", "estrategia en uso", "pierde", fecha="2026-11-25")]
        count = count_trials(rows, BASE, date(2026, 11, 25))
        self.assertEqual(count.strategy, ["R099"])
        self.assertEqual(count.expected, BASE + 4)
        ok, _ = check(config(rule={"dsr_trials": BASE + 3}), rows)
        self.assertFalse(ok)


class CheckTest(unittest.TestCase):
    def test_equal_passes(self) -> None:
        ok, _ = check(config(rule={"dsr_trials": BASE + 3}), ROWS)
        self.assertTrue(ok)

    def test_base_comes_from_the_config(self) -> None:
        other = {**BUDGET_SECTION, "dsr_trials_base": BASE + 1}
        ok, _ = check({"rule": {"dsr_trials": BASE + 3}, "alpha_budget": other}, ROWS)
        self.assertFalse(ok)

    def test_missing_budget_section_fails(self) -> None:
        ok, _ = check({"rule": {"dsr_trials": BASE + 3}}, ROWS)
        self.assertFalse(ok)

    def test_frozen_but_not_run_passes_with_notice(self) -> None:
        ok, lines = check(config(rule={"dsr_trials": BASE + 5}), ROWS)
        self.assertTrue(ok)
        self.assertTrue(any(line.startswith("aviso") for line in lines))

    def test_uncounted_test_fails(self) -> None:
        ok, _ = check(config(rule={"dsr_trials": BASE + 2}), ROWS)
        self.assertFalse(ok)

    def test_decreasing_value_fails(self) -> None:
        ok, _ = check(config(a={"dsr_trials": BASE + 4}, b={"dsr_trials": BASE + 3}), ROWS)
        self.assertFalse(ok)

    def test_no_dsr_trials_fails(self) -> None:
        ok, _ = check(config(orb={"range_candles": 2}), ROWS)
        self.assertFalse(ok)


HEADER = (
    "| Fecha | Hipótesis | Pre-registro / Issue | α_j | Costo α_j/(1 − α_j) | Premio | W antes "
    "| W después |\n|---|---|---|---|---|---|---|---|\n"
)
OPEN = "| 2026-11-25 | W(0): inicio de la época | #690 | — | — | — | — | 4,7500 % |\n"


def ledger(*lines: str) -> str:
    return f"texto\n{LEDGER_START}\n{HEADER}{OPEN}{''.join(lines)}{LEDGER_END}\n"


def bet(day: str, a: str, cost: str, before: str, after: str, payout: str = "—") -> str:
    return f"| {day} | H | #1 | {a} | {cost} | {payout} | {before} | {after} |\n"


FOUR_BETS = [
    bet("2026-11-26", "1,2500 %", "1,2658 %", "4,7500 %", "3,4842 %"),
    bet("2026-11-27", "1,2500 %", "1,2658 %", "3,4842 %", "2,2184 %"),
    bet("2026-11-28", "1,2500 %", "1,2658 %", "2,2184 %", "0,9525 %"),
    # La 4.ª solo puede apostar W/(1 + W) y deja W en 0.
    bet("2026-11-29", "0,9435 %", "0,9525 %", "0,9525 %", "0,0000 %"),
]


class LedgerTest(unittest.TestCase):
    def test_percent(self) -> None:
        self.assertEqual(parse_percent("1,25 %"), Decimal("0.0125"))
        self.assertIsNone(parse_percent("—"))

    def test_opening_row_alone_passes(self) -> None:
        ok, lines = check_ledger(BUDGET, parse_ledger(ledger()))
        self.assertTrue(ok, lines)

    def test_four_bets_without_discoveries_reach_zero(self) -> None:
        ok, lines = check_ledger(BUDGET, parse_ledger(ledger(*FOUR_BETS)))
        self.assertTrue(ok, lines)

    def test_a_fifth_bet_with_nothing_left_fails(self) -> None:
        extra = bet("2026-11-30", "0,1000 %", "0,1001 %", "0,0000 %", "-0,1001 %")
        ok, _ = check_ledger(BUDGET, parse_ledger(ledger(*FOUR_BETS, extra)))
        self.assertFalse(ok)

    def test_bet_above_the_rule_fails(self) -> None:
        line = bet("2026-11-26", "2,5000 %", "2,5641 %", "4,7500 %", "2,1859 %")
        ok, _ = check_ledger(BUDGET, parse_ledger(ledger(line)))
        self.assertFalse(ok)

    def test_wrong_arithmetic_fails(self) -> None:
        line = bet("2026-11-26", "1,2500 %", "1,2500 %", "4,7500 %", "3,5000 %")
        ok, _ = check_ledger(BUDGET, parse_ledger(ledger(line)))
        self.assertFalse(ok)

    def test_confirmed_discovery_earns_the_payout(self) -> None:
        line = bet("2026-11-26", "1,2500 %", "1,2658 %", "4,7500 %", "8,4842 %", "5,0000 %")
        ok, lines = check_ledger(BUDGET, parse_ledger(ledger(line)))
        self.assertTrue(ok, lines)

    def test_payout_above_omega_fails(self) -> None:
        line = bet("2026-11-26", "1,2500 %", "1,2658 %", "4,7500 %", "9,4842 %", "6,0000 %")
        ok, _ = check_ledger(BUDGET, parse_ledger(ledger(line)))
        self.assertFalse(ok)

    def test_broken_chain_fails(self) -> None:
        lines = [FOUR_BETS[0], bet("2026-11-27", "1,2500 %", "1,2658 %", "4,7500 %", "3,4842 %")]
        ok, _ = check_ledger(BUDGET, parse_ledger(ledger(*lines)))
        self.assertFalse(ok)

    def test_dates_going_back_fail(self) -> None:
        lines = [FOUR_BETS[1].replace("2026-11-27", "2026-11-24")]
        ok, _ = check_ledger(BUDGET, parse_ledger(ledger(*lines)))
        self.assertFalse(ok)

    def test_epoch_must_match_the_config(self) -> None:
        moved = load_budget({"alpha_budget": {**BUDGET_SECTION, "start": "2026-12-01"}})
        self.assertEqual(moved.start, date(2026, 12, 1))
        ok, _ = check_ledger(moved, parse_ledger(ledger()))
        self.assertFalse(ok)

    def test_missing_markers_fail(self) -> None:
        with self.assertRaises(ValueError):
            parse_ledger("sin libro")


class RepositoryTest(unittest.TestCase):
    def test_repository_files_reconcile(self) -> None:
        self.assertEqual(main([]), 0)


if __name__ == "__main__":
    unittest.main()
