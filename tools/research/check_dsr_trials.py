"""Check the deflated-Sharpe N and the global alpha budget ledger (#660, #690).

Rule approved by the owner on 25-11-2026 (docs/analysis/2026-11-regla-presupuesto-global.md):

1. Deflated-Sharpe N (§4):

    expected N = base ([alpha_budget] dsr_trials_base of config/research.toml, 344)
               + rows of the register with programa == "ML"
               + voting hypothesis rows: programa starts with "programa", does not contain
                 "descriptivo", and the verdict does not say the run was replaced
                 ("reemplazada", an invalid run whose hypothesis is counted by the row
                 that replaces it)
               + rows of the strategy in use (programa starts with "estrategia en uso")
                 dated on or after the epoch start (point 11: parameter changes and new
                 walk-forwards of the strategy in use count from the approval on; the older
                 ones are inside the base).

   The base are the trials counted before the register existed (round 1 pre-registration,
   §2.5). The latest `dsr_trials` (in file order the last) must be >= the expected N: a
   smaller value means a test was recorded without being counted. A larger value is allowed
   (hypotheses frozen but not yet run) and is reported. Values must never decrease.

2. Alpha-investing ledger (§2.5 and §5): the table between the markers
   `<!-- libro-alfa:inicio -->` and `<!-- libro-alfa:fin -->` of
   docs/analysis/registro-de-resultados.md ("Presupuesto global"). The first row opens the
   epoch with W(0) = initial_wealth on `start`. Each later row is one frozen hypothesis:

       bet α_j <= min(max_bet, W/(1 + W)),  cost = α_j/(1 − α_j),  0 <= payout <= ω,
       W after = W before − cost + payout >= 0,

   with "W antes" equal to the previous row's "W después", and dates never going back.
   Shown values are percentages with up to 4 decimals; the check recomputes the chain from
   the shown bets and payouts and accepts a rounding difference of 0.0001 percentage points.

Exit code 0 = ok, 1 = mismatch. Standard library only (Python >= 3.11).
"""

from __future__ import annotations

import argparse
import csv
import sys
import tomllib
from dataclasses import dataclass
from datetime import date
from decimal import Decimal, InvalidOperation
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_CONFIG = ROOT / "config" / "research.toml"
DEFAULT_REGISTER = ROOT / "docs" / "analysis" / "registro-de-resultados.csv"
DEFAULT_LEDGER = ROOT / "docs" / "analysis" / "registro-de-resultados.md"

LEDGER_START = "<!-- libro-alfa:inicio -->"
LEDGER_END = "<!-- libro-alfa:fin -->"
# Rounding tolerance of the shown percentages (4 decimals), as a fraction.
TOLERANCE = Decimal("0.000001")
EMPTY = {"", "—", "-"}


@dataclass(frozen=True)
class Count:
    base: int
    ml: list[str]
    hypotheses: list[str]
    replaced: list[str]
    strategy: list[str]

    @property
    def expected(self) -> int:
        return self.base + len(self.ml) + len(self.hypotheses) + len(self.strategy)


def count_trials(rows: list[dict[str, str]], base: int, since: date | None = None) -> Count:
    ml: list[str] = []
    hypotheses: list[str] = []
    replaced: list[str] = []
    strategy: list[str] = []
    for row in rows:
        program = row["programa"].strip()
        verdict = row["veredicto"].strip().lower()
        if program.startswith("estrategia en uso"):
            if since is not None and date.fromisoformat(row["fecha"].strip()) >= since:
                strategy.append(row["id"])
        elif program == "ML":
            ml.append(row["id"])
        elif program.startswith("programa") and "descriptivo" not in program:
            if "reemplazada" in verdict:
                replaced.append(row["id"])
            else:
                hypotheses.append(row["id"])
    return Count(base, ml, hypotheses, replaced, strategy)


def dsr_trials_in_order(config: dict[str, object]) -> list[tuple[str, int]]:
    found: list[tuple[str, int]] = []
    for name, section in config.items():
        if isinstance(section, dict) and "dsr_trials" in section:
            value = section["dsr_trials"]
            if not isinstance(value, int):
                raise ValueError(f"[{name}] dsr_trials no es un entero: {value!r}")
            found.append((name, value))
    return found


@dataclass(frozen=True)
class Budget:
    alpha: Decimal
    initial_wealth: Decimal
    start: date
    max_bet: Decimal
    payout: Decimal
    dsr_trials_base: int

    def max_bet_for(self, wealth: Decimal) -> Decimal:
        if wealth <= 0:
            return Decimal(0)
        return min(self.max_bet, wealth / (1 + wealth))


def load_budget(config: dict[str, object]) -> Budget:
    section = config.get("alpha_budget")
    if not isinstance(section, dict):
        raise ValueError("config/research.toml no tiene la sección [alpha_budget]")
    base = section.get("dsr_trials_base")
    if not isinstance(base, int):
        raise ValueError(f"[alpha_budget] dsr_trials_base no es un entero: {base!r}")
    return Budget(
        alpha=Decimal(section["alpha"]),
        initial_wealth=Decimal(section["initial_wealth"]),
        start=date.fromisoformat(section["start"]),
        max_bet=Decimal(section["max_bet"]),
        payout=Decimal(section["payout"]),
        dsr_trials_base=base,
    )


def check(config: dict[str, object], rows: list[dict[str, str]]) -> tuple[bool, list[str]]:
    lines: list[str] = []
    ok = True
    try:
        budget = load_budget(config)
    except (ValueError, KeyError, InvalidOperation) as e:
        return False, [f"ERROR: {e}"]
    sections = dsr_trials_in_order(config)
    if not sections:
        return False, ["config/research.toml no tiene ningún dsr_trials"]
    for (prev_name, prev), (name, value) in zip(sections, sections[1:]):
        if value < prev:
            ok = False
            lines.append(f"ERROR: [{name}] dsr_trials = {value} baja frente a [{prev_name}] = {prev}")
    last_name, latest = sections[-1]
    count = count_trials(rows, budget.dsr_trials_base, budget.start)
    lines.append(
        f"N esperado = {count.base} (base, [alpha_budget]) + {len(count.ml)} ML"
        f" ({', '.join(count.ml)}) + {len(count.hypotheses)} hipótesis que votan"
        f" ({', '.join(count.hypotheses)}) + {len(count.strategy)} filas de la estrategia en uso"
        f" desde {budget.start} ({', '.join(count.strategy)}) = {count.expected}"
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


# ── ledger ──────────────────────────────────────────────────────────────────────────────────


@dataclass(frozen=True)
class LedgerRow:
    day: date
    hypothesis: str
    bet: Decimal | None
    cost: Decimal | None
    payout: Decimal | None
    before: Decimal | None
    after: Decimal


def parse_percent(text: str) -> Decimal | None:
    """'1,2500 %' -> Decimal('0.0125'); '—' -> None."""
    t = text.strip()
    if t in EMPTY:
        return None
    t = t.removesuffix("%").strip().replace(",", ".")
    try:
        return Decimal(t) / 100
    except InvalidOperation as e:
        raise ValueError(f"no es un porcentaje: {text!r}") from e


def parse_ledger(markdown: str) -> list[LedgerRow]:
    if LEDGER_START not in markdown or LEDGER_END not in markdown:
        raise ValueError(f"no se encontró el libro ({LEDGER_START} … {LEDGER_END})")
    block = markdown.split(LEDGER_START, 1)[1].split(LEDGER_END, 1)[0]
    table = [line.strip() for line in block.splitlines() if line.strip().startswith("|")]
    if len(table) < 2:
        raise ValueError("el libro no tiene tabla")
    rows: list[LedgerRow] = []
    for line in table[2:]:  # header and separator
        cells = [c.strip() for c in line.strip("|").split("|")]
        if len(cells) != 8:
            raise ValueError(f"fila con {len(cells)} columnas (se esperan 8): {line}")
        day, hypothesis, _source, bet, cost, payout, before, after = cells
        after_value = parse_percent(after)
        if after_value is None:
            raise ValueError(f"fila sin 'W después': {line}")
        rows.append(
            LedgerRow(
                day=date.fromisoformat(day),
                hypothesis=hypothesis,
                bet=parse_percent(bet),
                cost=parse_percent(cost),
                payout=parse_percent(payout),
                before=parse_percent(before),
                after=after_value,
            )
        )
    return rows


def pct(x: Decimal) -> str:
    return f"{(x * 100).quantize(Decimal('0.0001'))} %"


def check_ledger(budget: Budget, rows: list[LedgerRow]) -> tuple[bool, list[str]]:
    if not rows:
        return False, ["ERROR: el libro del presupuesto global está vacío"]
    errors: list[str] = []
    first = rows[0]
    if first.bet is not None or first.cost is not None:
        errors.append("la primera fila abre la época: no lleva apuesta ni costo")
    if first.day != budget.start:
        errors.append(f"la época empieza el {budget.start}, no el {first.day}")
    if abs(first.after - budget.initial_wealth) > TOLERANCE:
        errors.append(f"W(0) = {pct(first.after)}, la config dice {pct(budget.initial_wealth)}")
    wealth = budget.initial_wealth  # exact chain
    shown = first.after
    previous_day = first.day
    for row in rows[1:]:
        name = f"{row.day} {row.hypothesis}"
        if row.day < previous_day:
            errors.append(f"{name}: la fecha va para atrás")
        previous_day = row.day
        if row.bet is None or row.bet <= 0:
            errors.append(f"{name}: falta la apuesta α_j")
            continue
        if row.before is None or abs(row.before - shown) > TOLERANCE:
            errors.append(f"{name}: 'W antes' no es el 'W después' de la fila anterior")
        limit = budget.max_bet_for(wealth)
        if row.bet > limit + TOLERANCE:
            errors.append(
                f"{name}: apuesta {pct(row.bet)} mayor que mín(máximo, W/(1 + W)) = {pct(limit)}"
            )
        cost = row.bet / (1 - row.bet)
        if row.cost is None or abs(row.cost - cost) > TOLERANCE:
            errors.append(f"{name}: el costo tiene que ser α_j/(1 − α_j) = {pct(cost)}")
        payout = row.payout or Decimal(0)
        if payout < 0 or payout > budget.payout:
            errors.append(f"{name}: premio {pct(payout)} fuera de [0, ω = {pct(budget.payout)}]")
        wealth = wealth - cost + payout
        if wealth < -TOLERANCE:
            errors.append(f"{name}: W queda negativo ({pct(wealth)})")
        if abs(row.after - wealth) > TOLERANCE:
            errors.append(f"{name}: 'W después' = {pct(row.after)}, la cuenta da {pct(wealth)}")
        shown = row.after
    lines = [f"ERROR (libro): {e}" for e in errors]
    lines.append(
        f"presupuesto global: {len(rows) - 1} hipótesis desde {budget.start}, "
        f"W = {pct(wealth)}; próxima apuesta máxima {pct(budget.max_bet_for(wealth))}"
    )
    return not errors, lines


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument("--register", type=Path, default=DEFAULT_REGISTER)
    parser.add_argument("--ledger", type=Path, default=DEFAULT_LEDGER)
    args = parser.parse_args(argv)
    with args.config.open("rb") as f:
        config = tomllib.load(f)
    with args.register.open(newline="", encoding="utf-8") as f:
        rows = list(csv.DictReader(f))
    ok, lines = check(config, rows)
    try:
        budget = load_budget(config)
        ledger = parse_ledger(args.ledger.read_text(encoding="utf-8"))
        ledger_ok, ledger_lines = check_ledger(budget, ledger)
    except (ValueError, KeyError, InvalidOperation) as e:
        ledger_ok, ledger_lines = False, [f"ERROR (libro): {e}"]
    for line in lines + ledger_lines:
        print(line)
    return 0 if ok and ledger_ok else 1


if __name__ == "__main__":
    sys.exit(main())
