"""Guardado de noticias en PostgreSQL (#111): tabla `news_items`, de solo agregar.

Una noticia es (fuente, id). Leerla otra vez, desde el mismo feed u otro de la misma fuente, no
la duplica. Si vuelve con otro título, enlace u hora de publicación, es un conflicto: no se pisa
lo guardado y el lote entero se descarta (todo o nada).
"""

from collections.abc import Sequence
from dataclasses import dataclass

import psycopg

from cfds_intel.news.feeds import NewsItem


class NewsConflictError(ValueError):
    """La misma noticia ya está guardada con otros datos."""


@dataclass(frozen=True)
class InsertReport:
    inserted: int
    unchanged: int


def insert_news(conn: psycopg.Connection, items: Sequence[NewsItem]) -> InsertReport:
    """Guarda `items` en una sola transacción."""
    inserted = unchanged = 0
    with conn.transaction(), conn.cursor() as cur:
        for n in items:
            cur.execute(
                """
                INSERT INTO news_items
                    (source, feed, item_id, title, url, published_at, retrieved_at)
                VALUES (%s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (source, item_id) DO NOTHING
                """,
                (n.source, n.feed, n.item_id, n.title, n.url, n.published_at, n.retrieved_at),
            )
            if cur.rowcount == 1:
                inserted += 1
                continue
            cur.execute(
                "SELECT title, url, published_at FROM news_items"
                " WHERE source = %s AND item_id = %s",
                (n.source, n.item_id),
            )
            row = cur.fetchone()
            if row is not None and tuple(row) == (n.title, n.url, n.published_at):
                unchanged += 1
            else:
                raise NewsConflictError(
                    f"{n.source}/{n.item_id} ya está guardada con otros datos: no se pisa"
                )
    return InsertReport(inserted, unchanged)
