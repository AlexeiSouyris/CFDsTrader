"""Lee los feeds de `config/news.toml` y guarda las noticias (#111).

```text
DATABASE_URL=... uv run python -m cfds_intel.news [ruta/a/config/news.toml]
```
Un feed que no cumple las reglas se informa y no se guarda nada de él; los demás siguen.
"""

import os
import sys
from pathlib import Path

import psycopg

from cfds_intel.news.feeds import FeedError, fetch, load_config
from cfds_intel.news.store import NewsConflictError, insert_news

DEFAULT_CONFIG = Path(__file__).resolve().parents[4] / "config" / "news.toml"


def main(argv: list[str]) -> int:
    cfg = load_config(Path(argv[1] if len(argv) > 1 else DEFAULT_CONFIG).read_text())
    failed = 0
    with psycopg.connect(os.environ["DATABASE_URL"]) as conn:
        for feed in cfg.feeds:
            try:
                r = insert_news(conn, fetch(feed, cfg))
            except (FeedError, NewsConflictError, OSError) as e:
                failed += 1
                print(f"⚠️  {feed.source}/{feed.name}: {e}", file=sys.stderr)
                continue
            print(f"{feed.source}/{feed.name}: {r.inserted} nuevas, {r.unchanged} ya estaban")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
