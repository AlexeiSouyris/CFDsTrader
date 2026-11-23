"""Estados emocionales que salen del índice de sentimiento (spec §9.1, #220).

| Estado          | Regla                                                   |
|-----------------|---------------------------------------------------------|
| miedo extremo   | índice <= -0,6                                          |
| euforia extrema | índice >= +0,6                                          |
| complacencia    | percentil de la volatilidad realizada <= 0,10           |

- Umbrales en `config/sentiment.toml` [states].
- Miedo y euforia extremos solo existen si el índice se emitió (mínimo de componentes).
- La complacencia usa el percentil **sin invertir** de la volatilidad realizada (baja = calma),
  que calcula el índice (#213). Existe aunque el índice no se haya emitido, siempre que ese
  componente sea válido. No se recalcula nada: se leen los valores del índice.
- Los estados que salen del precio (FOMO, clímax de venta, rechazo, distribución) son de Rust.
"""

import tomllib
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation

from cfds_intel.sentiment.index import NotEmitted, SentimentConfigError, SentimentIndex

EXTREME_FEAR = "extreme_fear"
EXTREME_EUPHORIA = "extreme_euphoria"
COMPLACENCY = "complacency"


@dataclass(frozen=True)
class StatesConfig:
    extreme_fear: Decimal
    extreme_euphoria: Decimal
    complacency_percentile: Decimal


def load_states_config(text: str) -> StatesConfig:
    try:
        raw = tomllib.loads(text)["states"]
        if set(raw) - {"extreme_fear", "extreme_euphoria", "complacency_percentile"}:
            raise SentimentConfigError("states: claves desconocidas")
        cfg = StatesConfig(
            Decimal(str(raw["extreme_fear"])),
            Decimal(str(raw["extreme_euphoria"])),
            Decimal(str(raw["complacency_percentile"])),
        )
    except (KeyError, TypeError, InvalidOperation, tomllib.TOMLDecodeError) as e:
        raise SentimentConfigError(f"config/sentiment.toml [states] inválido: {e!r}") from e
    if not -1 <= cfg.extreme_fear < 0 < cfg.extreme_euphoria <= 1:
        raise SentimentConfigError("states: hace falta -1 <= extreme_fear < 0 < euphoria <= 1")
    if not 0 < cfg.complacency_percentile < 1:
        raise SentimentConfigError("states: complacency_percentile tiene que estar en (0, 1)")
    return cfg


def index_states(r: SentimentIndex | NotEmitted, cfg: StatesConfig) -> tuple[str, ...]:
    """Estados activos en ese momento, en un orden fijo."""
    out = []
    if isinstance(r, SentimentIndex):
        if r.value <= cfg.extreme_fear:
            out.append(EXTREME_FEAR)
        if r.value >= cfg.extreme_euphoria:
            out.append(EXTREME_EUPHORIA)
    vol = next((c for c in r.components if c.name == "realized_vol"), None)
    if (
        vol is not None
        and vol.status == "valid"
        and vol.percentile is not None
        and vol.percentile <= cfg.complacency_percentile
    ):
        out.append(COMPLACENCY)
    return tuple(out)
