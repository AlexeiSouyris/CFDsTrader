"""Reporte de aprobación de la fase 4b (ROADMAP, #221): ¿cada capa mejora frente a no usarla,
sin fuga de información entre períodos?

- **Noticias** (ajuste del puntaje, spec §9): dos corridas de `backtest_run` iguales en todo
  salvo la capa (`--sin-noticias` → `news: null` en `backtest_runs`). Se compara el P/L neto
  **por semana ISO** (de `signal_at`, en UTC) de cada corrida en las ventanas de **validación**
  walk-forward (`config/data_split.toml`), con el mismo bootstrap por semanas de la regla de ML
  §6 (`compare.block_interval`). Mejora si la diferencia media (con - sin) es positiva y el
  intervalo queda entero por encima de 0. El P/L lo calculó Rust (`outcomes.net_pnl`): acá solo
  se suma por semana, en `Decimal`.
- **Análogos**: solo informativos (spec §13), no entran al puntaje: no hay "sin análogos" que
  comparar.
- **ML**: informativo (spec ML §2). Su regla de mejora (M1 contra B1) ya la registra el
  experimento (`ml_experiments`): acá se lee, no se recalcula.

El tramo fuera de muestra final **no se usa**: ni operaciones decididas en él ni resueltas en él
(queda para la fase 5, una sola vez). Funciones puras salvo `load_run` y `ml_result`.
"""

import json
from collections import defaultdict
from dataclasses import dataclass
from datetime import datetime, timedelta
from decimal import Decimal
from typing import Any, Literal

import psycopg

from cfds_intel.models.compare import block_interval, week_of
from cfds_intel.models.splits import DataSplit, SplitConfig, SplitError, plan

Status = Literal["improves", "no_improvement", "insufficient", "not_applicable"]
LABEL: dict[Status, str] = {
    "improves": "mejora",
    "no_improvement": "no mejora",
    "insufficient": "datos insuficientes",
    "not_applicable": "no aplica (solo informativa)",
}


@dataclass(frozen=True)
class Trade:
    signal_at: datetime
    resolved_at: datetime  # última salida: desde ahí se conoce el resultado
    net_pnl: Decimal  # moneda del instrumento (lo calculó Rust)


@dataclass(frozen=True)
class Run:
    run_id: str
    instruments: tuple[str, ...]
    from_at: datetime
    to_at: datetime
    config_sha256: str
    news: dict[str, Any] | None
    trades: tuple[Trade, ...]


@dataclass(frozen=True)
class Bootstrap:
    resamples: int
    confidence: float
    seed: int


@dataclass(frozen=True)
class NewsResult:
    status: Status
    reason: str | None
    weeks: int = 0
    trades_on: int = 0
    trades_off: int = 0
    pnl_on: Decimal = Decimal(0)
    pnl_off: Decimal = Decimal(0)
    excluded_oos: int = 0
    difference: Decimal | None = None  # media semanal, con - sin
    low: Decimal | None = None
    high: Decimal | None = None
    oos_start: datetime | None = None


def _validation_weeks(split: DataSplit) -> list[tuple[int, int]]:
    weeks: set[tuple[int, int]] = set()
    for w in split.windows:
        day = w.validation.start
        while day < w.validation.end:
            weeks.add(week_of(day))
            day += timedelta(days=1)
    return sorted(weeks)


def _in_validation(t: datetime, split: DataSplit) -> bool:
    return any(w.validation.start <= t < w.validation.end for w in split.windows)


def compare_news(
    on: Run, off: Run, split_cfg: SplitConfig, boot: Bootstrap, min_weeks: int
) -> NewsResult:
    """Capa de noticias: corrida con ajuste (`on`) contra sin ajuste (`off`)."""
    same = (on.instruments, on.from_at, on.to_at, on.config_sha256) == (
        off.instruments,
        off.from_at,
        off.to_at,
        off.config_sha256,
    )
    if not same:
        raise ValueError("las dos corridas tienen que ser iguales salvo la capa de noticias")
    if on.news is None or off.news is not None:
        raise ValueError("`on` tiene que tener noticias y `off` no (backtest_run --sin-noticias)")
    if len(on.instruments) != 1:
        raise ValueError("una corrida de un solo instrumento: el P/L va en su moneda")
    try:
        split = plan(on.from_at, on.to_at, split_cfg)
    except SplitError as e:
        return NewsResult("insufficient", f"sin ventanas walk-forward: {e}")
    oos = split.out_of_sample.start
    if not on.news.get("interpretations"):
        return NewsResult(
            "insufficient",
            "la corrida con noticias no tuvo ninguna interpretación: la capa no se pudo usar",
            oos_start=oos,
        )
    weeks = _validation_weeks(split)

    def weekly(run: Run) -> tuple[dict[tuple[int, int], Decimal], int, int]:
        by_week: dict[tuple[int, int], Decimal] = defaultdict(Decimal)
        used = excluded = 0
        for t in run.trades:
            if not _in_validation(t.signal_at, split):
                continue
            if t.resolved_at >= oos:  # se resolvió fuera de muestra: no se mira
                excluded += 1
                continue
            by_week[week_of(t.signal_at)] += t.net_pnl
            used += 1
        return by_week, used, excluded

    pnl_on, n_on, x_on = weekly(on)
    pnl_off, n_off, x_off = weekly(off)
    base = NewsResult(
        "insufficient",
        None,
        weeks=len(weeks),
        trades_on=n_on,
        trades_off=n_off,
        pnl_on=sum(pnl_on.values(), Decimal(0)),
        pnl_off=sum(pnl_off.values(), Decimal(0)),
        excluded_oos=x_on + x_off,
        oos_start=oos,
    )
    if len(weeks) < min_weeks:
        return _with(base, reason=f"{len(weeks)} semanas de validación (mínimo {min_weeks})")
    if n_on == 0 and n_off == 0:
        return _with(base, reason="ninguna operación en las ventanas de validación")
    # Todas las semanas de validación cuentan, también las sin operaciones (P/L 0).
    diffs = [pnl_on.get(w, Decimal(0)) - pnl_off.get(w, Decimal(0)) for w in weeks]
    low, high = block_interval(diffs, [1] * len(diffs), boot.resamples, boot.confidence, boot.seed)
    mean = sum(diffs, Decimal(0)) / len(diffs)
    status: Status = "improves" if mean > 0 and low > 0 else "no_improvement"
    return _with(base, status=status, difference=mean, low=low, high=high)


def _with(r: NewsResult, **changes: Any) -> NewsResult:
    return NewsResult(**{**r.__dict__, **changes})


@dataclass(frozen=True)
class MlResult:
    status: Status
    reason: str | None
    experiment_id: str | None = None
    difference: float | None = None  # log loss M1 - B1 (negativa: M1 pierde menos)
    low: float | None = None
    high: float | None = None
    blocks: int | None = None
    trained: int = 0
    skipped: int = 0
    purged: int = 0
    embargoed: int = 0


ML_STATUS: dict[str, Status] = {
    "m1_improves": "improves",
    "b1_stays": "no_improvement",
    "suspended": "insufficient",
}


def ml_from_document(doc: dict[str, Any] | None) -> MlResult:
    """Resultado de ML desde el documento de `ml_experiment.v1` (o `None`: no hay)."""
    if doc is None:
        return MlResult("insufficient", "sin experimento registrado para la corrida con noticias")
    r, windows = doc["result"], doc["windows"]
    status = ML_STATUS[r["status"]]
    return MlResult(
        status,
        "ninguna ventana llega al mínimo de datos (spec ML §6)"
        if status == "insufficient"
        else None,
        experiment_id=doc["experiment_id"],
        difference=r["difference"],
        low=r["low"],
        high=r["high"],
        blocks=r["blocks"],
        trained=sum(1 for w in windows if w["skipped"] is None),
        skipped=sum(1 for w in windows if w["skipped"] is not None),
        purged=sum(w["purged"] for w in windows),
        embargoed=sum(w["embargoed"] for w in windows),
    )


def load_run(conn: psycopg.Connection, run_id: str) -> Run:
    row = conn.execute(
        """SELECT instruments::text, from_at, to_at, config_sha256, news::text
           FROM backtest_runs WHERE run_id = %s""",
        (run_id,),
    ).fetchone()
    if row is None:
        raise ValueError(f"la corrida {run_id!r} no está en backtest_runs")
    trades = []
    for signal_at, net_pnl, exits in conn.execute(
        """SELECT p.signal_at, o.net_pnl, o.exits::text
           FROM proposals p JOIN outcomes o ON o.proposal_id = p.id AND o.kind = 'simulated'
           WHERE p.run_id = %s ORDER BY p.signal_at""",
        (run_id,),
    ).fetchall():
        times = [datetime.fromisoformat(e["at"].replace("Z", "+00:00")) for e in json.loads(exits)]
        trades.append(Trade(signal_at, max(times, default=signal_at), Decimal(net_pnl)))
    return Run(
        run_id,
        tuple(json.loads(row[0])),
        row[1],
        row[2],
        row[3],
        None if row[4] is None else json.loads(row[4]),
        tuple(trades),
    )


def ml_result(conn: psycopg.Connection, run_id: str) -> MlResult:
    """El último experimento registrado de la corrida."""
    row = conn.execute(
        """SELECT document::text FROM ml_experiments WHERE backtest_run_id = %s
           ORDER BY created_at DESC, experiment_id LIMIT 1""",
        (run_id,),
    ).fetchone()
    return ml_from_document(None if row is None else json.loads(row[0]))


def _money(d: Decimal | None) -> str:
    return "—" if d is None else f"{d.quantize(Decimal('0.01'))}"


def _num(x: float | None) -> str:
    return "—" if x is None else f"{x:+.4f}"


def render(
    on: Run,
    off: Run,
    news: NewsResult,
    ml: MlResult,
    boot: Bootstrap,
    generated_at: datetime,
    code_version: str,
) -> str:
    """El informe en markdown."""
    conf = f"{boot.confidence:.0%}"
    cut = on.news.get("published_from") if on.news else None
    rows = [
        ("Noticias (ajuste del puntaje)", news.status),
        ("Análogos históricos", "not_applicable"),
        ("ML v1 (M1 contra B1)", ml.status),
    ]
    out = [
        "# Reporte de aprobación de la fase 4b",
        "",
        f"- Generado: {generated_at:%Y-%m-%d %H:%M} UTC · código `{code_version}`",
        f"- Corridas: `{on.run_id}` (con noticias) y `{off.run_id}` (sin noticias), "
        f"{', '.join(on.instruments)}, del {on.from_at:%Y-%m-%d} al {on.to_at:%Y-%m-%d}, "
        f"configuración `{on.config_sha256[:12]}`",
        "- Se mide solo en las ventanas de **validación** walk-forward. El tramo fuera de muestra"
        + (f" (desde {news.oos_start:%Y-%m-%d})" if news.oos_start else "")
        + " no se usa: queda para la fase 5.",
        "",
        "## Conclusión por capa",
        "",
        "| Capa | Conclusión |",
        "|---|---|",
        *(f"| {name} | **{LABEL[s]}** |" for name, s in rows),
        "",
        "## Noticias",
        "",
        "Diferencia del P/L neto por semana (con - sin), en la moneda del instrumento. Mejora si"
        f" la media es positiva y el intervalo del {conf} (bootstrap por semanas,"
        f" {boot.resamples} remuestreos) queda entero por encima de 0.",
        "",
    ]
    if news.reason:
        out += [f"**Datos insuficientes:** {news.reason}.", ""]
    out += [
        "| | Con noticias | Sin noticias |",
        "|---|---|---|",
        f"| Operaciones en validación | {news.trades_on} | {news.trades_off} |",
        f"| P/L neto en validación | {_money(news.pnl_on)} | {_money(news.pnl_off)} |",
        "",
        f"- Semanas de validación: {news.weeks}",
        f"- Diferencia media por semana: {_money(news.difference)}"
        f" · intervalo {conf}: [{_money(news.low)}; {_money(news.high)}]",
        f"- Operaciones descartadas por resolverse fuera de muestra: {news.excluded_oos}",
        "",
        "## Análogos históricos",
        "",
        "Solo informativos (spec §13, `config/analogs.toml`): no suman al puntaje ni filtran"
        " propuestas, así que no hay una variante sin análogos que comparar. Para que cuenten"
        " hace falta un voto (spec §15b), y entonces se miden acá.",
        "",
        "## ML v1",
        "",
        "Informativo (spec ML §2). Regla de §6: M1 mejora a B1 si su log loss es menor y el"
        " intervalo de la diferencia queda entero por debajo de 0.",
        "",
    ]
    if ml.reason:
        out += [f"**Datos insuficientes:** {ml.reason}.", ""]
    out += [
        f"- Experimento: `{(ml.experiment_id or '—')[:12]}`",
        f"- Log loss M1 - B1: {_num(ml.difference)} · intervalo: [{_num(ml.low)}; {_num(ml.high)}]"
        f" · semanas: {ml.blocks if ml.blocks is not None else '—'}",
        f"- Ventanas entrenadas: {ml.trained} · sin el mínimo de datos: {ml.skipped}",
        "",
        "## Fuga de información (leakage)",
        "",
        "- Noticias: solo las publicadas desde el corte del intérprete"
        + (f" ({cut})" if cut else "")
        + "; en cada vela, solo lo interpretado hasta su cierre (motor, spec §9).",
        "- Cada operación se asigna a la semana de su señal; las resueltas fuera de muestra no se"
        " cuentan.",
        f"- ML: propuestas purgadas {ml.purged} y embargadas {ml.embargoed} en el entrenamiento"
        " (spec ML §5); el tramo fuera de muestra nunca se entrega al modelo.",
        "",
        "Las probabilidades e intervalos describen el backtest, no prometen resultados.",
        "",
    ]
    return "\n".join(out)
