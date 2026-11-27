"""Tipos de cambio diarios del BCE (ADR 0013 §4, #418), guardados como `fx_rate.v1`.

```text
DATABASE_URL=... uv run python -m cfds_intel.rates.fx [desde AAAA-MM-DD]
```
Sin fecha, carga cada par desde su `start` (`config/fx_rates.toml`). No gasta cupo de IG.

```text
ref_date     = TIME_PERIOD (el día del tipo)
published_at = ese mismo día a la hora de publicación de config (nunca antes)
```
A diferencia del €STR, el BCE publica el tipo de cambio el mismo día. Se reusan el formato de
config, la descarga y la lectura del CSV de `cfds_intel.rates.ecb`.

La tabla `fx_rates` es de solo agregar. El mismo valor para el mismo par y fecha no se repite;
otro valor es una **corrección** de la fuente: fila nueva, publicada cuando se leyó.
"""

import os
import sys
from collections.abc import Sequence
from dataclasses import dataclass, replace
from datetime import UTC, date, datetime
from decimal import Decimal
from pathlib import Path

import psycopg

from cfds_intel.rates.ecb import RatesError, SeriesConfig, fetch_csv, load_config, parse_csv

CONFIG = Path(__file__).resolve().parents[4] / "config" / "fx_rates.toml"


@dataclass(frozen=True)
class FxRate:
    """Un documento de `fx_rate.v1`."""

    pair: str
    ref_date: date
    published_at: datetime
    rate: Decimal

    def to_contract(self) -> dict[str, str]:
        return {
            "pair": self.pair,
            "ref_date": self.ref_date.isoformat(),
            "published_at": self.published_at.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "rate": str(self.rate),
        }


def to_fx_rates(values: Sequence[tuple[date, Decimal]], s: SeriesConfig) -> list[FxRate]:
    """(fecha, tipo) → registros publicados ese día a la hora de config. Tipo > 0 o error."""
    if len(s.name) != 6 or not s.name.isupper():
        raise RatesError(f"{s.name}: el par tiene que ser BASE+COTIZADA, por ejemplo EURUSD")
    out: list[FxRate] = []
    for day, value in values:
        if not value > 0:
            raise RatesError(f"{s.name} {day}: tipo de cambio no positivo {value}")
        published = datetime.combine(day, s.publication_time, s.publication_tz).astimezone(UTC)
        out.append(FxRate(s.name, day, published, value))
    return out


@dataclass(frozen=True)
class StoreReport:
    inserted: int
    unchanged: int
    corrections: int


def insert_fx_rates(
    conn: psycopg.Connection, rates: Sequence[FxRate], retrieved_at: datetime
) -> StoreReport:
    """Todo o nada: una transacción por lote."""
    inserted = unchanged = corrections = 0
    with conn.transaction(), conn.cursor() as cur:
        for r in rates:
            cur.execute(
                """
                SELECT rate FROM fx_rates
                WHERE pair = %s AND ref_date = %s
                ORDER BY published_at DESC LIMIT 1
                """,
                (r.pair, r.ref_date),
            )
            last = cur.fetchone()
            if last is not None and last[0] == r.rate:
                unchanged += 1
                continue
            row = r if last is None else replace(r, published_at=max(retrieved_at, r.published_at))
            cur.execute(
                """
                INSERT INTO fx_rates (pair, ref_date, published_at, rate)
                VALUES (%s, %s, %s, %s)
                """,
                (row.pair, row.ref_date, row.published_at, row.rate),
            )
            if last is None:
                inserted += 1
            else:
                corrections += 1
    return StoreReport(inserted, unchanged, corrections)


def main(argv: list[str]) -> int:
    cfg = load_config(CONFIG.read_text())
    since_arg = date.fromisoformat(argv[1]) if len(argv) > 1 else None
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        for s in cfg.series.values():
            since = max(since_arg, s.start) if since_arg else s.start
            values = parse_csv(fetch_csv(cfg, s, since), s)
            now = datetime.now(UTC)
            # Un valor todavía no publicado según config no se guarda (nunca se adelanta).
            rates = [r for r in to_fx_rates(values, s) if r.published_at <= now]
            r = insert_fx_rates(conn, rates, now)
            span = f"{values[0][0]} → {values[-1][0]}" if values else "sin datos"
            print(
                f"{s.name}: {len(values)} días ({span}) → "
                f"{r.inserted} nuevos, {r.unchanged} ya estaban, {r.corrections} correcciones"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
