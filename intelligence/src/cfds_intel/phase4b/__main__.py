"""Reporte de aprobación de la fase 4b (#221). No usa IG: lee velas ya guardadas.

```text
# Todo: las dos corridas de backtest_run (con y sin noticias), el experimento de ML sobre la
# corrida con noticias y el informe.
DATABASE_URL=... uv run python -m cfds_intel.phase4b run \\
    <INSTRUMENTO> <desde UTC> <hasta UTC> <capital> <tamaño B&H> <run_id> [<informe.md>]

# Solo el informe, con corridas y experimento ya guardados.
DATABASE_URL=... uv run python -m cfds_intel.phase4b report \\
    <run_id con noticias> <run_id sin noticias> [<informe.md>]
```
`run` guarda las corridas como `<run_id>-noticias` y `<run_id>-sin-noticias`. El informe va a
`fase-4b-<run_id>.md` si no se indica otro archivo.
"""

import os
import subprocess
import sys
import tomllib
from datetime import UTC, datetime
from pathlib import Path

import psycopg

from cfds_intel.models.config import ROOT, load_config
from cfds_intel.models.dataset import BacktestOutcomes
from cfds_intel.models.experiment import CONFIG_FILES, code_version
from cfds_intel.models.experiment import run as run_experiment
from cfds_intel.models.registry import record
from cfds_intel.models.splits import load_split_config
from cfds_intel.phase4b.report import Bootstrap, compare_news, load_run, ml_result, render

USAGE = (
    "uso: python -m cfds_intel.phase4b run <INSTRUMENTO> <desde> <hasta> <capital> <tamaño> "
    "<run_id> [<informe.md>]\n"
    "     python -m cfds_intel.phase4b report <run con noticias> <run sin noticias> [<informe.md>]"
)


def min_weeks(text: str) -> int:
    v = tomllib.loads(text).get("news", {}).get("min_weeks")
    if not (isinstance(v, int) and not isinstance(v, bool) and v >= 1):
        raise ValueError("config/phase4b.toml: news.min_weeks tiene que ser un entero ≥ 1")
    return v


def report(conn: psycopg.Connection, on_id: str, off_id: str) -> str:
    texts = {n: (ROOT / "config" / n).read_text() for n in ("ml.toml", "data_split.toml")}
    cfg = load_config(texts["ml.toml"])
    boot = Bootstrap(
        cfg.evaluation.bootstrap_resamples, cfg.evaluation.confidence, cfg.training.seed
    )
    on, off = load_run(conn, on_id), load_run(conn, off_id)
    news = compare_news(
        on,
        off,
        load_split_config(texts["data_split.toml"]),
        boot,
        min_weeks((ROOT / "config" / "phase4b.toml").read_text()),
    )
    return render(on, off, news, ml_result(conn, on_id), boot, datetime.now(UTC), code_version())


def backtest(args: list[str], run_id: str, without_news: bool) -> None:
    cmd = ["cargo", "run", "--quiet", "-p", "cfds-backtest", "--example", "backtest_run", "--"]
    cmd += [*args, run_id] + (["--sin-noticias"] if without_news else [])
    # Lista de argumentos, sin shell: nada se interpreta.
    subprocess.run(cmd, cwd=ROOT / "engine", check=True)  # noqa: S603


def main(argv: list[str]) -> int:
    if len(argv) >= 7 and argv[0] == "run":
        bt, run_id, out = argv[1:6], argv[6], argv[7] if len(argv) > 7 else None
        on_id, off_id = f"{run_id}-noticias", f"{run_id}-sin-noticias"
        backtest(bt, on_id, without_news=False)
        backtest(bt, off_id, without_news=True)
        texts = {n: (ROOT / "config" / n).read_text() for n in CONFIG_FILES}
        with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
            try:
                _, _, doc, _ = run_experiment(
                    conn, BacktestOutcomes(on_id), load_config(texts["ml.toml"]), texts
                )
                record(conn, doc)
            except ValueError as e:  # dataset o cortes imposibles: el informe lo dice
                print(f"ML: sin experimento ({e})", file=sys.stderr)
            text = report(conn, on_id, off_id)
    elif len(argv) in (3, 4) and argv[0] == "report":
        on_id, off_id, out = argv[1], argv[2], argv[3] if len(argv) > 3 else None
        run_id = on_id
        with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
            text = report(conn, on_id, off_id)
    else:
        print(USAGE, file=sys.stderr)
        return 2
    path = Path(out or f"fase-4b-{run_id}.md")
    path.write_text(text)
    print(text)
    print(f"informe: {path}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
