"""Interpreta las noticias nuevas con el intérprete fijado, las guarda y las publica (#186).

```text
DATABASE_URL=... REDIS_URL=... uv run python -m cfds_intel.news.interpret_new [--limit N]
```

- Toma de `news_items` las noticias sin interpretación del intérprete fijado en
  `config/news_interpreter.toml` (Ollama, modelo y digest), de la más vieja a la más nueva.
- Antes de interpretar nada comprueba Ollama (modelo instalado y digest), PostgreSQL y Redis. Si
  algo falla, termina con un mensaje claro sin guardar nada.
- Cada noticia se guarda y publica en su propia transacción: nunca queda una a medias.
  - Si Ollama, PostgreSQL o Redis fallan en el medio, se corta ahí; las ya guardadas quedan y la
    próxima corrida sigue desde la siguiente.
  - Si falla solo esa noticia (no se pudo leer su texto oficial o la respuesta no cumple las
    reglas de `interpret()`), se avisa, se sigue con las demás y se reintenta en la próxima corrida.
- No está en el `crontab` ni se usa en el backtest (lo decide el dueño).
"""

import argparse
import os
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from datetime import UTC, datetime
from pathlib import Path

import psycopg
import redis

from cfds_intel.news.feeds import NewsItem
from cfds_intel.news.fulltext import FullTextError
from cfds_intel.news.interpret import (
    Interpretation,
    InterpretationError,
    Interpreter,
    instrument_ids,
    interpret,
)
from cfds_intel.news.interpretations import RedisPublisher, pending, save
from cfds_intel.news.ollama import (
    OllamaError,
    OllamaInterpreter,
    Transport,
    load_config,
    local_http,
)

ROOT = Path(__file__).resolve().parents[4]
DEFAULT_CONFIG = ROOT / "config" / "news_interpreter.toml"
DEFAULT_INSTRUMENTS = ROOT / "config" / "instruments.toml"


class AbortError(RuntimeError):
    """Algo que no es de una sola noticia (Ollama, PostgreSQL o Redis): se corta la corrida."""


@dataclass
class Report:
    saved: int = 0
    already: int = 0
    skipped: list[str] = field(default_factory=list)


def run(
    items: Sequence[NewsItem],
    interpreter: Interpreter,
    instruments: Sequence[str],
    store: Callable[[Interpretation], bool],
    now: Callable[[], datetime] = lambda: datetime.now(UTC),
) -> Report:
    """Interpreta y guarda cada noticia. `store` guarda y publica una (True si era nueva)."""
    report = Report()
    for item in items:
        key = f"{item.source}/{item.item_id}"
        try:
            interp = interpret(item, interpreter, instruments, now())
        except OllamaError as e:
            raise AbortError(f"Ollama falló en {key} ({report.saved} nuevas antes): {e}") from e
        except (FullTextError, InterpretationError) as e:
            report.skipped.append(f"{key}: {e}")
            continue
        except OSError as e:
            raise AbortError(f"Ollama no responde ({key}): {e}") from e
        try:
            new = store(interp)
        except (psycopg.Error, redis.RedisError) as e:
            raise AbortError(f"no se pudo guardar o publicar {key}: {e}") from e
        if new:
            report.saved += 1
        else:
            report.already += 1
    return report


def main(argv: list[str], transport: Transport = local_http) -> int:
    """`transport` habla con Ollama; se reemplaza en los tests."""
    ap = argparse.ArgumentParser(prog="python -m cfds_intel.news.interpret_new")
    ap.add_argument("--limit", type=int, default=None, help="máximo de noticias en esta corrida")
    ap.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    ap.add_argument("--instruments", type=Path, default=DEFAULT_INSTRUMENTS)
    args = ap.parse_args(argv[1:])
    if args.limit is not None and args.limit < 1:
        ap.error("--limit tiene que ser ≥ 1")

    cfg = load_config(args.config.read_text())
    instruments = instrument_ids(args.instruments.read_text())
    try:
        interpreter = OllamaInterpreter(cfg, transport=transport)
    except OllamaError as e:
        print(f"❌ Ollama: {e}. No se guardó nada.", file=sys.stderr)
        return 2
    except OSError as e:
        print(f"❌ Ollama no responde en {cfg.url}: {e}. No se guardó nada.", file=sys.stderr)
        return 2
    publisher = RedisPublisher(os.environ["REDIS_URL"])
    try:
        publisher.ping()
    except redis.RedisError as e:
        print(f"❌ Redis no responde: {e}. No se guardó nada.", file=sys.stderr)
        return 2

    with psycopg.connect(os.environ["DATABASE_URL"], autocommit=True) as conn:
        items = pending(conn, interpreter.info, args.limit)
        print(f"{len(items)} noticias sin interpretar con {interpreter.info.model}")
        try:
            report = run(items, interpreter, instruments, lambda i: save(conn, i, publisher))
        except AbortError as e:
            print(f"❌ {e}. Se corta la corrida; lo ya guardado queda.", file=sys.stderr)
            return 1
    for s in report.skipped:
        print(f"⚠️  {s}", file=sys.stderr)
    print(
        f"{report.saved} guardadas y publicadas en {publisher.stream}, "
        f"{report.already} ya estaban, {len(report.skipped)} salteadas"
    )
    return 1 if report.skipped else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
