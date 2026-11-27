"""Guardia de los tests que escriben en PostgreSQL (#376): nunca en la base real.

Los tests que escriben en la base llevan la marca `pg` (`pytestmark = pytest.mark.pg`). Antes
de correr cada uno:
- sin `DATABASE_URL`, se saltea (como antes);
- si el nombre de la base no empieza con `cfds_test`, **falla** sin conectarse, con un mensaje
  claro. Así correrlos con el `DATABASE_URL` del `.env` (la base real) no escribe nada.
"""

from __future__ import annotations

import os
from urllib.parse import urlsplit

import pytest

# `pytester` corre un pytest aparte para probar la guardia (test_db_guard.py).
pytest_plugins = ["pytester"]

TEST_DB_PREFIX = "cfds_test"


def database_name(url: str) -> str | None:
    """El nombre de la base de una URL de PostgreSQL (`postgresql://u:p@h:5432/<base>?...`)."""
    if "://" in url:
        name = urlsplit(url).path.lstrip("/")
        return name or None
    # Cadena de conexión con palabras clave de libpq: "host=... dbname=...".
    for part in url.split():
        key, _, value = part.partition("=")
        if key == "dbname" and value:
            return value
    return None


def check_test_database(url: str) -> str | None:
    """`None` si la URL apunta a una base de pruebas; si no, el mensaje de error."""
    name = database_name(url)
    if name is None:
        return (
            "DATABASE_URL sin nombre de base: los tests necesitan una base cuyo nombre empiece "
            f"con {TEST_DB_PREFIX!r} (#376)"
        )
    if not name.startswith(TEST_DB_PREFIX):
        return (
            f"los tests no escriben en la base real: DATABASE_URL apunta a la base {name!r}. "
            f"Usá una base cuyo nombre empiece con {TEST_DB_PREFIX!r} (por ejemplo "
            "cfds_test_<tu-nombre>, creada con createdb) o dejá estos tests para el CI (#376)"
        )
    return None


def pytest_configure(config: pytest.Config) -> None:
    config.addinivalue_line(
        "markers", "pg: escribe en PostgreSQL; solo corre contra una base cfds_test* (#376)"
    )


@pytest.hookimpl(tryfirst=True)
def pytest_runtest_setup(item: pytest.Item) -> None:
    if item.get_closest_marker("pg") is None:
        return
    url = os.environ.get("DATABASE_URL")
    if url is None:
        pytest.skip("necesita PostgreSQL con migraciones (DATABASE_URL)")
    error = check_test_database(url)
    if error is not None:
        pytest.fail(error, pytrace=False)
