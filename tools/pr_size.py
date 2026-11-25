#!/usr/bin/env python3
"""Aviso de tamaño de PR (CLAUDE.md §2, #669): no bloquea, solo avisa.

Cuenta las líneas cambiadas (agregadas + borradas) entre la base y la punta del PR, sin contar
tests, fixtures, lockfiles, datos ni las fichas de `docs/modules/`. Si pasan el límite, escribe
una anotación `::warning::` de GitHub Actions. Siempre termina con 0.

Uso:
    python3 tools/pr_size.py <sha-base> <sha-punta> [--limit 400]

Solo usa la biblioteca estándar de Python.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys

# Rutas que no cuentan. Se comparan contra la ruta completa del archivo.
EXCLUDED = [
    r"(^|/)tests?/",  # carpetas de tests (Rust, Python, ui/src/test)
    r"(^|/)__tests__/",
    r"(^|/)fixtures?/",
    r"(^|/)test_[^/]*$",  # test_algo.py
    r"_test\.[^/]+$",  # algo_test.rs
    r"\.(test|spec)\.[^/]+$",  # algo.test.tsx
    r"(^|/)(Cargo\.lock|uv\.lock|package-lock\.json|poetry\.lock|yarn\.lock|pnpm-lock\.yaml)$",
    r"\.(csv|parquet|jsonl|ndjson|snap)$",  # datos
    r"(^|/)data/",
    r"^docs/modules/",
    r"^graphify-out/",
]
_EXCLUDED_RE = re.compile("|".join(EXCLUDED))


def counted(path: str) -> bool:
    return not _EXCLUDED_RE.search(path)


def numstat(base: str, head: str) -> list[tuple[int, int, str]]:
    out = subprocess.run(
        ["git", "diff", "--numstat", "--no-renames", f"{base}...{head}"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    rows = []
    for line in out.splitlines():
        added, deleted, path = line.split("\t", 2)
        if added == "-":  # binario
            continue
        rows.append((int(added), int(deleted), path))
    return rows


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("base")
    parser.add_argument("head")
    parser.add_argument("--limit", type=int, default=400)
    args = parser.parse_args(argv)
    try:
        rows = numstat(args.base, args.head)
    except subprocess.CalledProcessError as e:
        print(f"::warning::No pude medir el tamaño del PR: {e.stderr.strip()}")
        return 0
    total = sum(a + d for a, d, p in rows if counted(p))
    excluded = sum(a + d for a, d, p in rows if not counted(p))
    print(f"Líneas cambiadas que cuentan: {total} (límite {args.limit}); sin contar: {excluded}.")
    if total > args.limit:
        top = sorted((r for r in rows if counted(r[2])), key=lambda r: -(r[0] + r[1]))[:5]
        detail = ", ".join(f"{p} ({a + d})" for a, d, p in top)
        print(
            f"::warning title=PR grande::Este PR cambia {total} líneas sin contar tests, fixtures, "
            f"lockfiles, datos ni docs/modules (CLAUDE.md §2 pide menos de {args.limit}). "
            f"Si se puede, partilo. Los que más cambian: {detail}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
