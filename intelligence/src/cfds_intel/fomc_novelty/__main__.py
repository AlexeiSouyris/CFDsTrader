"""Mide la novedad de cada comunicado del FOMC respecto del anterior (#668).

```text
# Sin guardar: lee los comunicados de official_releases (solo lectura), pide las páginas que
# falten en [client] html_dir y escribe la serie en un CSV.
DATABASE_URL=... uv run python -m cfds_intel.fomc_novelty --dry-run --csv /ruta/serie.csv
# Guardar en fomc_statement_novelty (migración 0050), en el Mac mini:
DATABASE_URL=... uv run python -m cfds_intel.fomc_novelty
# También los embeddings (hace falta `ollama pull nomic-embed-text` antes):
DATABASE_URL=... uv run python -m cfds_intel.fomc_novelty \
    --method word_diff --method embedding_cosine
```
Unas 120 páginas de federalreserve.gov, de a una y con `pause_secs` entre pedidos (ADR 0016
§1.2); las que ya están en `html_dir` no se vuelven a pedir. Un 403, 404 o 429 frena sin
reintentar. Es idempotente: otra corrida igual no agrega nada. No gasta cupo de IG.
"""

import argparse
import csv
import os
import sys
from collections.abc import Callable
from datetime import UTC, datetime
from pathlib import Path

import psycopg

from cfds_intel.calendar.official.model import OfficialCalendarError, utc_z
from cfds_intel.calendar.official.pages import PoliteFetcher
from cfds_intel.fomc_novelty.embed import cosine_distance, embed, model_digest
from cfds_intel.fomc_novelty.model import (
    MeasuredText,
    NoveltyConfig,
    NoveltyError,
    NoveltyRow,
    Statement,
    load_config,
)
from cfds_intel.fomc_novelty.series import embedding_rows, word_diff_rows
from cfds_intel.fomc_novelty.store import insert_rows, statements
from cfds_intel.fomc_novelty.text import measure

REPO_ROOT = Path(__file__).resolve().parents[4]
DEFAULT_CONFIG = REPO_ROOT / "config" / "fomc_novelty.toml"


def cached_getter(html_dir: Path, get: Callable[[str], str]) -> Callable[[str], str]:
    """Lee la página de `html_dir` si ya está; si no, la pide y la guarda."""

    def read(url: str) -> str:
        path = html_dir / url.rsplit("/", 1)[-1]
        if path.exists():
            return path.read_text(encoding="utf-8")
        body = get(url)
        html_dir.mkdir(parents=True, exist_ok=True)
        path.write_text(body, encoding="utf-8")
        return body

    return read


def measure_all(
    cfg: NoveltyConfig, found: list[Statement], get: Callable[[str], str]
) -> list[MeasuredText]:
    out = []
    for s in found:
        if not s.source_url.startswith(cfg.client.release_url_prefix):
            raise NoveltyError(f"{s.source_url}: no es una página de comunicado documentada")
        out.append(measure(s, get(s.source_url), cfg.end_marker))
    return out


def write_csv(path: Path, rows: list[NoveltyRow]) -> None:
    with path.open("w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(
            [
                "reference_period",
                "status",
                "available_at",
                "previous_reference_period",
                "method",
                "method_version",
                "model",
                "model_digest",
                "value",
                "word_count",
                "previous_word_count",
            ]
        )
        for r in rows:
            w.writerow(
                [
                    r.reference_period,
                    r.status,
                    utc_z(r.available_at),
                    r.previous_reference_period,
                    r.method,
                    r.method_version,
                    r.model or "",
                    r.model_digest or "",
                    r.value,
                    r.word_count,
                    r.previous_word_count,
                ]
            )


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(prog="python -m cfds_intel.fomc_novelty")
    ap.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    ap.add_argument(
        "--method",
        action="append",
        choices=("word_diff", "embedding_cosine"),
        help="por defecto, solo word_diff",
    )
    ap.add_argument("--dry-run", action="store_true", help="no guarda en la base")
    ap.add_argument("--csv", type=Path, default=None, help="escribe la serie en este CSV")
    args = ap.parse_args(argv[1:])
    cfg = load_config(args.config.read_text())
    methods = args.method or ["word_diff"]
    html_dir = Path(cfg.client.html_dir)
    if not html_dir.is_absolute():
        html_dir = REPO_ROOT / html_dir
    fetcher = PoliteFetcher(
        cfg.client.user_agent,
        cfg.client.timeout_secs,
        cfg.client.pause_secs,
        (cfg.client.release_url_prefix,),
    )
    now = datetime.now(UTC)
    try:
        with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
            conn.read_only = True
            found = statements(conn, cfg.first_day, cfg.last_day, cfg.statuses)
        if len(found) < 2:
            raise NoveltyError("menos de dos comunicados en official_releases: ¿se cargó?")
        texts = measure_all(cfg, found, cached_getter(html_dir, fetcher))
        rows = []
        if "word_diff" in methods:
            rows += word_diff_rows(texts, cfg.word_diff_version, now)
        if "embedding_cosine" in methods:
            e = cfg.embedding
            digest = model_digest(e)
            rows += embedding_rows(
                texts,
                lambda t: embed(e, t),
                cosine_distance,
                e.model,
                digest,
                e.method_version,
                now,
            )
    except (NoveltyError, OfficialCalendarError) as err:
        print(f"⚠️  fomc_novelty: {err}", file=sys.stderr)
        return 1
    finally:
        print(f"fomc_novelty: {fetcher.requests} páginas pedidas", file=sys.stderr)
    print(f"fomc_novelty: {len(texts)} comunicados, {len(rows)} filas ({', '.join(methods)})")
    if args.csv:
        write_csv(args.csv, rows)
        print(f"fomc_novelty: serie en {args.csv}")
    if args.dry_run:
        return 0
    try:
        with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
            rep = insert_rows(conn, rows)
    except NoveltyError as err:
        print(f"⚠️  fomc_novelty: {err}", file=sys.stderr)
        return 1
    print(f"fomc_novelty: {rep.inserted} nuevas, {rep.unchanged} ya estaban")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
