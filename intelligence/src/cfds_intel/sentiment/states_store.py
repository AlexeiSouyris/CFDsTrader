"""Guardado de los estados emocionales del índice (spec §9.1, #220) en `emotional_states`.

Contrato `emotional_state.v1` (migración 0033), la misma tabla donde el motor guarda los de
precio. Acá solo los del índice: `source = sentiment`, `basis = sentiment_index`.

- Un registro por cada valor del índice (y por cada momento sin índice, donde solo puede haber
  complacencia), con `as_of` = cuándo se conoció: el `published_at` del índice.
- También sin estados (`states` vacío): así se sabe que se evaluó y no había ninguno.
- De solo agregar: lo mismo otra vez no hace nada; el mismo momento con otros estados es un error
  (sería repintado) y no se guarda nada del lote.
"""

import json
from collections.abc import Sequence
from dataclasses import dataclass
from datetime import datetime

import psycopg

from cfds_intel.sentiment.index import NotEmitted, SentimentIndex
from cfds_intel.sentiment.states import StatesConfig, index_state_values

SOURCE = "sentiment"
BASIS = "sentiment_index"


class EmotionRepaintError(RuntimeError):
    """Ya hay estados guardados para ese momento y son otros."""


@dataclass(frozen=True)
class StatesRow:
    instrument_id: str
    as_of: datetime
    states: tuple[tuple[str, dict[str, str]], ...]

    def states_json(self) -> list[dict[str, object]]:
        return [{"state": name, "values": values} for name, values in self.states]

    def to_contract(self) -> dict[str, object]:
        return {
            "instrument_id": self.instrument_id,
            "source": SOURCE,
            "basis": BASIS,
            "as_of": self.as_of.isoformat().replace("+00:00", "Z"),
            "states": self.states_json(),
        }


@dataclass(frozen=True)
class StatesStoreReport:
    inserted: int
    unchanged: int


def states_rows(
    emitted: Sequence[SentimentIndex], skipped: Sequence[NotEmitted], cfg: StatesConfig
) -> list[StatesRow]:
    """Los registros de una corrida del índice, en orden de tiempo. Si en un mismo momento hay
    índice y "sin índice", vale el índice."""
    by_time: dict[tuple[str, datetime], StatesRow] = {}
    for n in skipped:
        by_time[(n.instrument_id, n.at)] = StatesRow(
            n.instrument_id, n.at, index_state_values(n, cfg)
        )
    for r in emitted:
        by_time[(r.instrument_id, r.published_at)] = StatesRow(
            r.instrument_id, r.published_at, index_state_values(r, cfg)
        )
    return [by_time[k] for k in sorted(by_time, key=lambda k: (k[1], k[0]))]


def insert_states(conn: psycopg.Connection, rows: Sequence[StatesRow]) -> StatesStoreReport:
    inserted = unchanged = 0
    with conn.transaction(), conn.cursor() as cur:
        for r in rows:
            states = r.states_json()
            cur.execute(
                """
                INSERT INTO emotional_states (instrument_id, source, basis, as_of, states)
                VALUES (%s, %s, %s, %s, %s)
                ON CONFLICT ON CONSTRAINT emotional_states_key DO NOTHING
                """,
                (r.instrument_id, SOURCE, BASIS, r.as_of, json.dumps(states)),
            )
            if cur.rowcount == 1:
                inserted += 1
                continue
            cur.execute(
                """
                SELECT states FROM emotional_states
                WHERE instrument_id = %s AND source = %s AND basis = %s AND as_of = %s
                """,
                (r.instrument_id, SOURCE, BASIS, r.as_of),
            )
            stored = cur.fetchone()
            if stored is None or stored[0] != states:
                raise EmotionRepaintError(
                    f"{r.instrument_id} {r.as_of.isoformat()}: los estados ya guardados son otros "
                    "(sería repintado; revisar el índice o la configuración)"
                )
            unchanged += 1
    return StatesStoreReport(inserted, unchanged)
