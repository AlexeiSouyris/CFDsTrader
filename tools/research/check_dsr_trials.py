"""Check that `dsr_trials` in config/research.toml matches the results register (#660).

Rule (proposal docs/analysis/2026-11-regla-presupuesto-global.md §4, PENDING owner approval):

    expected N = N_BASE
               + rows of the register with programa == "ML"
               + voting hypothesis rows: programa starts with "programa", does not contain
                 "descriptivo", and the verdict does not say the run was replaced
                 ("reemplazada", an invalid run whose hypothesis is counted by the row
                 that replaces it).

N_BASE = 344 are the trials counted before the register existed (round 1 pre-registration,
§2.5: ~300 cuts of #461, 11 DE40 variants of #373 and the walk-forwards of the strategy in
use, rounded up). 350 (round 1) = 344 + ML v1 + 3 ML v2 families + C + D1.

The latest `dsr_trials` (the largest, in file order the last) must be >= the expected N:
a smaller value means a test was recorded without being counted. A larger value is allowed
(hypotheses frozen but not yet run) and is reported. Values must never decrease in file order.

Exit code 0 = ok, 1 = mismatch. Standard library only (Python >= 3.11).
"""

from __future__ import annotations

import argparse
import csv
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path

N_BASE = 344

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_CONFIG = ROOT / "config" / "research.toml"
DEFAULT_REGISTER = ROOT / "docs" / "analysis" / "registro-de-resultados.csv"


@dataclass(frozen=True)
class Count:
    ml: list[str]
    hypotheses: list[str]
    replaced: list[str]

    @property
    def expected(self) -> int:
        return N_BASE + len(self.ml) + len(self.hypotheses)


def count_trials(rows: list[dict[str, str]]) -> Count:
    ml: list[str] = []
    hypotheses: list[str] = []
    replaced: list[str] = []
    for row in rows:
        program = row["programa"].strip()
        verdict = row["veredicto"].strip().lower()
        if program == "ML":
            ml.append(row["id"])
        elif program.startswith("programa") and "descriptivo" not in program:
            if "reemplazada" in verdict:
                replaced.append(row["id"])
            else:
                hypotheses.append(row["id"])
    return Count(ml, hypotheses, replaced)


def dsr_trials_in_order(config: dict[str, object]) -> list[tuple[str, int]]:
    found: list[tuple[str, int]] = []
    for name, section in config.items():
        if isinstance(section, dict) and "dsr_trials" in section:
            value = section["dsr_trials"]
            if not isinstance(value, int):
                raise ValueError(f"[{name}] dsr_trials no es un entero: {value!r}")
            found.append((name, value))
    return found


def check(config: dict[str, object], rows: list[dict[str, str]]) -> tuple[bool, list[str]]:
    lines: list[str] = []
    ok = True
    sections = dsr_trials_in_order(config)
    if not sections:
        return False, ["config/research.toml no tiene ningún dsr_trials"]
    for (prev_name, prev), (name, value) in zip(sections, sections[1:]):
        if value < prev:
            ok = False
            lines.append(f"ERROR: [{name}] dsr_trials = {value} baja frente a [{prev_name}] = {prev}")
    last_name, latest = sections[-1]
    count = count_trials(rows)
    lines.append(
        f"N esperado = {N_BASE} (base) + {len(count.ml)} ML ({', '.join(count.ml)})"
        f" + {len(count.hypotheses)} hipótesis que votan ({', '.join(count.hypotheses)})"
        f" = {count.expected}"
    )
    if count.replaced:
        lines.append(f"corridas reemplazadas, no suman: {', '.join(count.replaced)}")
    lines.append(f"último dsr_trials: [{last_name}] = {latest}")
    if latest < count.expected:
        ok = False
        lines.append(
            f"ERROR: el registro tiene {count.expected - latest} prueba(s) más que el último "
            "dsr_trials. Una prueba se registró sin sumarla al N del Sharpe deflactado."
        )
    elif latest > count.expected:
        lines.append(
            f"aviso: dsr_trials supera al registro en {latest - count.expected}: hipótesis "
            "congeladas que todavía no tienen fila (se aceptan)."
        )
    else:
        lines.append("ok: coinciden")
    return ok, lines


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument("--register", type=Path, default=DEFAULT_REGISTER)
    args = parser.parse_args(argv)
    with args.config.open("rb") as f:
        config = tomllib.load(f)
    with args.register.open(newline="", encoding="utf-8") as f:
        rows = list(csv.DictReader(f))
    ok, lines = check(config, rows)
    for line in lines:
        print(line)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
