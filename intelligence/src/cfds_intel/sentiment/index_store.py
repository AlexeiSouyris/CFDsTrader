"""Lectura de los valores crudos y guardado del índice de sentimiento v1 (spec §9.1, #213).

- **Lee** lo que guardaron otros, sin recalcular nada:
  - COT: `sentiment_observations` (Python, #113);
  - volatilidad realizada: `daily_indicators` (Rust, #148);
  - caída desde el máximo: `daily_drawdowns` (Rust, #213).

  Un valor en calentamiento (`NULL`) no es un dato y no se lee.
- **Guarda** en `sentiment_index`, de solo agregar, todo o nada por lote. Un registro idéntico ya
  guardado cuenta como "ya estaba". Otro valor para el mismo instrumento y hora es repintado:
  error, y no se guarda nada.
"""

import json
from collections.abc import Sequence
from dataclasses import dataclass
from datetime import UTC, date, datetime, time
from decimal import Decimal

import psycopg

from cfds_intel.sentiment.cot import SOURCE as COT_SOURCE
from cfds_intel.sentiment.index import (
    COT_LONG,
    COT_OPEN_INTEREST,
    COT_SHORT,
    Point,
    SentimentIndex,
    cot_points,
)


class SentimentRepaintError(RuntimeError):
    """Un índice ya guardado daría otro valor: sería repintado. No se guarda nada."""


@dataclass(frozen=True)
class IndexStoreReport:
    inserted: int
    unchanged: int


def load_points(
    conn: psycopg.Connection,
    instrument: str,
    realized_vol_window: int,
    atr_period: int,
    drawdown_window: int,
) -> dict[str, list[Point]]:
    """Los datos crudos de cada componente con datos de `instrument`."""
    cot_rows = conn.execute(
        """
        SELECT series, observed_for, published_at, value FROM sentiment_observations
        WHERE source = %s AND instrument_id = %s AND series IN (%s, %s, %s)
        """,
        (COT_SOURCE, instrument, COT_LONG, COT_SHORT, COT_OPEN_INTEREST),
    ).fetchall()
    vol = conn.execute(
        """
        SELECT day, published_at, realized_vol FROM daily_indicators
        WHERE instrument_id = %s AND realized_vol_window = %s AND atr_period = %s
          AND realized_vol IS NOT NULL
        ORDER BY day
        """,
        (instrument, realized_vol_window, atr_period),
    ).fetchall()
    dd = conn.execute(
        """
        SELECT day, published_at, drawdown FROM daily_drawdowns
        WHERE instrument_id = %s AND drawdown_window = %s AND drawdown IS NOT NULL
        ORDER BY day
        """,
        (instrument, drawdown_window),
    ).fetchall()
    return {
        "cot": cot_points((s, o, p, v) for s, o, p, v in cot_rows),
        "realized_vol": [_daily(d, p, v) for d, p, v in vol],
        "drawdown": [_daily(d, p, v) for d, p, v in dd],
    }


def insert_index(conn: psycopg.Connection, records: Sequence[SentimentIndex]) -> IndexStoreReport:
    inserted = unchanged = 0
    with conn.transaction(), conn.cursor() as cur:
        for r in records:
            doc = r.to_contract()
            cur.execute(
                """
                INSERT INTO sentiment_index (instrument_id, as_of, published_at, value,
                    warmup_complete, min_components, components)
                VALUES (%s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (instrument_id, published_at) DO NOTHING
                """,
                (
                    r.instrument_id,
                    r.as_of,
                    r.published_at,
                    r.value,
                    r.warmup_complete,
                    r.min_components,
                    json.dumps(doc["components"]),
                ),
            )
            if cur.rowcount == 1:
                inserted += 1
                continue
            cur.execute(
                """
                SELECT as_of, value, warmup_complete, min_components, components
                FROM sentiment_index WHERE instrument_id = %s AND published_at = %s
                """,
                (r.instrument_id, r.published_at),
            )
            stored = cur.fetchone()
            same = stored is not None and (
                stored[0] == r.as_of
                and stored[1] == r.value
                and stored[2] == r.warmup_complete
                and stored[3] == r.min_components
                and stored[4] == doc["components"]
            )
            if not same:
                raise SentimentRepaintError(
                    f"{r.instrument_id} {doc['published_at']}: el índice ya guardado es otro "
                    "(sería repintado; revisar los datos crudos o la configuración)"
                )
            unchanged += 1
    return IndexStoreReport(inserted, unchanged)


def _daily(day: date, published_at: datetime, value: Decimal) -> Point:
    """Un valor diario corresponde a su día (00:00 UTC) y vale desde el fin de la vela."""
    return Point(datetime.combine(day, time(0), UTC), published_at, value)
