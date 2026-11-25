#!/usr/bin/env python3
"""Genera las tablas índice de la documentación (#669).

- Registro de módulos de `docs/ARCHITECTURE.md`, desde el front-matter de `docs/modules/*.md`.
- Índice de contratos de `contracts/README.md`, desde `contracts/*.schema.json`.

Cada tabla vive entre dos marcadores HTML y no se edita a mano.

Uso (desde la raíz del repo):
    python3 tools/docs_index.py           # regenera las dos tablas
    python3 tools/docs_index.py --check   # falla si alguna no está al día (lo corre el CI)

Solo usa la biblioteca estándar de Python.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULES_DIR = ROOT / "docs" / "modules"
ARCHITECTURE = ROOT / "docs" / "ARCHITECTURE.md"
CONTRACTS_DIR = ROOT / "contracts"
CONTRACTS_README = CONTRACTS_DIR / "README.md"

REGEN_CMD = "python3 tools/docs_index.py"
MODULE_KEYS = ("modulo", "ruta", "lenguaje", "estado", "resumen")
MAX_SUMMARY = 160


class IndexError_(Exception):
    """Error de datos en una ficha o un contrato."""


def _cell(text: str) -> str:
    """Escapa lo que rompería una celda de tabla Markdown."""
    return text.replace("\n", " ").replace("|", "\\|").strip()


def parse_front_matter(path: Path) -> dict[str, str]:
    """Lee el front-matter `clave: valor` (una línea por clave) entre dos líneas `---`."""
    lines = path.read_text(encoding="utf-8").split("\n")
    if not lines or lines[0].strip() != "---":
        raise IndexError_(f"{path.relative_to(ROOT)}: falta el front-matter (primera línea `---`)")
    meta: dict[str, str] = {}
    for n, line in enumerate(lines[1:], start=2):
        if line.strip() == "---":
            break
        key, sep, value = line.partition(":")
        if not sep or not key.strip():
            raise IndexError_(f"{path.relative_to(ROOT)}:{n}: se esperaba `clave: valor`")
        meta[key.strip()] = value.strip()
    else:
        raise IndexError_(f"{path.relative_to(ROOT)}: el front-matter no cierra con `---`")
    missing = [k for k in MODULE_KEYS if not meta.get(k)]
    if missing:
        raise IndexError_(f"{path.relative_to(ROOT)}: faltan claves en el front-matter: {', '.join(missing)}")
    if len(meta["resumen"]) > MAX_SUMMARY:
        raise IndexError_(
            f"{path.relative_to(ROOT)}: `resumen` tiene {len(meta['resumen'])} caracteres "
            f"(máximo {MAX_SUMMARY}); el detalle va en el cuerpo de la ficha"
        )
    if path.stem != meta["modulo"]:
        raise IndexError_(f"{path.relative_to(ROOT)}: el nombre del archivo tiene que ser `{meta['modulo']}.md`")
    return meta


def modules_table() -> str:
    metas = [parse_front_matter(p) for p in sorted(MODULES_DIR.glob("*.md")) if p.name != "README.md"]
    # Orden por ruta: agrupa engine/, intelligence/, ui/... y no depende del orden de alta.
    metas.sort(key=lambda m: (m["ruta"].strip("`").lower(), m["modulo"]))
    rows = [
        "| Módulo | Ruta | Lenguaje | Responsabilidad (única) | Estado |",
        "|---|---|---|---|---|",
    ]
    for m in metas:
        rows.append(
            f"| [{m['modulo']}](modules/{m['modulo']}.md) | {_cell(m['ruta'])} | {_cell(m['lenguaje'])} "
            f"| {_cell(m['resumen'])} | {_cell(m['estado'])} |"
        )
    return "\n".join(rows)


def _first_sentence(text: str) -> str:
    """Primera oración de la descripción (hasta `. ` o `: `), recortada a una línea corta."""
    text = " ".join(text.split())
    match = re.search(r"[.:](\s|$)", text)
    sentence = text[: match.start()] if match else text
    if len(sentence) > MAX_SUMMARY:
        sentence = sentence[: MAX_SUMMARY - 1].rstrip() + "…"
    return sentence


def contracts_table() -> str:
    rows = [
        "| Contrato | Título | Qué es (primera oración de `description`) |",
        "|---|---|---|",
    ]
    for path in sorted(CONTRACTS_DIR.glob("*.schema.json")):
        try:
            schema = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as e:
            raise IndexError_(f"{path.relative_to(ROOT)}: JSON inválido: {e}") from e
        name = path.name.removesuffix(".schema.json")
        for key in ("$id", "title", "description"):
            if not schema.get(key):
                raise IndexError_(f"{path.relative_to(ROOT)}: falta `{key}`")
        if not schema["$id"].endswith("/" + path.name):
            raise IndexError_(f"{path.relative_to(ROOT)}: `$id` no termina en `/{path.name}`")
        rows.append(
            f"| [`{name}`]({path.name}) | {_cell(schema['title'])} | {_cell(_first_sentence(schema['description']))} |"
        )
    return "\n".join(rows)


def _markers(name: str) -> tuple[str, str]:
    return (
        f"<!-- {name}:inicio (generado por `{REGEN_CMD}`; no editar a mano) -->",
        f"<!-- {name}:fin -->",
    )


def replace_block(text: str, name: str, body: str, path: Path) -> str:
    start, end = _markers(name)
    i, j = text.find(start), text.find(end)
    if i < 0 or j < i:
        raise IndexError_(f"{path.relative_to(ROOT)}: faltan los marcadores `{start}` y `{end}`")
    return text[: i + len(start)] + "\n" + body + "\n" + text[j:]


TARGETS = (
    (ARCHITECTURE, "registro-de-modulos", modules_table),
    (CONTRACTS_README, "indice-de-contratos", contracts_table),
)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="no escribe; falla si alguna tabla no está al día")
    args = parser.parse_args(argv)
    stale: list[str] = []
    try:
        for path, name, build in TARGETS:
            current = path.read_text(encoding="utf-8")
            updated = replace_block(current, name, build(), path)
            if updated != current:
                stale.append(str(path.relative_to(ROOT)))
                if not args.check:
                    path.write_text(updated, encoding="utf-8")
    except IndexError_ as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    if args.check and stale:
        for s in stale:
            print(f"::error file={s}::La tabla generada no está al día. Corré `{REGEN_CMD}` y commiteá el resultado.")
        print(f"Tablas desactualizadas: {', '.join(stale)}. Regenerá con: {REGEN_CMD}", file=sys.stderr)
        return 1
    if not args.check:
        print("actualizado: " + (", ".join(stale) if stale else "nada (ya estaba al día)"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
