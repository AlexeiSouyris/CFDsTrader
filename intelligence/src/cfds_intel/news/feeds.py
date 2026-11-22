"""Lector de los RSS oficiales de la Fed y del BCE (#109, ADR 0006).

Reglas:
- La hora de una noticia es la que publica la fuente (`pubDate`), en UTC. Sin fecha, o con una
  fecha sin zona horaria, el ítem es un error: no se inventa una hora (CLAUDE.md §5b).
- Solo se guardan el título y el enlace: el texto completo no se republica.
- El XML llega de internet: se parsea con `defusedxml`, nunca con la librería estándar.
"""

import re
import tomllib
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from datetime import UTC, datetime
from email.utils import parsedate_to_datetime
from typing import Protocol

from defusedxml import DefusedXmlException, ElementTree

from cfds_intel.net import https_get

_LABEL = re.compile(r"^[a-z][a-z0-9_]{0,31}$")
_MAX_TEXT = 500
_MAX_URL = 1000


class FeedError(ValueError):
    """El feed o la configuración no cumplen las reglas: no se guarda nada de ese feed."""


@dataclass(frozen=True)
class FeedConfig:
    source: str
    name: str
    url: str


@dataclass(frozen=True)
class NewsConfig:
    user_agent: str
    timeout_secs: int
    feeds: tuple[FeedConfig, ...]


def load_config(text: str) -> NewsConfig:
    """Lee `config/news.toml` y lo valida."""
    raw = tomllib.loads(text)
    client = raw.get("client", {})
    user_agent = client.get("user_agent")
    timeout = client.get("timeout_secs")
    if not isinstance(user_agent, str) or not user_agent.strip():
        raise FeedError("client.user_agent tiene que ser un texto")
    if not isinstance(timeout, int) or isinstance(timeout, bool) or timeout <= 0:
        raise FeedError("client.timeout_secs tiene que ser un entero mayor que 0")
    feeds: list[FeedConfig] = []
    for f in raw.get("feeds", []):
        source, name, url = f.get("source"), f.get("name"), f.get("url")
        if not (isinstance(source, str) and _LABEL.match(source)):
            raise FeedError(f"feeds.source inválido: {source!r}")
        if not (isinstance(name, str) and _LABEL.match(name)):
            raise FeedError(f"feeds.name inválido: {name!r}")
        if not (isinstance(url, str) and url.startswith("https://")):
            raise FeedError(f"feeds.url tiene que ser https: {url!r}")
        feeds.append(FeedConfig(source, name, url))
    if not feeds:
        raise FeedError("no hay feeds configurados")
    if len({(f.source, f.name) for f in feeds}) != len(feeds):
        raise FeedError("feed repetido (misma fuente y nombre)")
    return NewsConfig(user_agent, timeout, tuple(feeds))


def _utc_z(t: datetime) -> str:
    return t.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


@dataclass(frozen=True)
class NewsItem:
    source: str
    feed: str
    item_id: str
    title: str
    url: str
    published_at: datetime
    retrieved_at: datetime

    def to_contract(self) -> dict[str, str]:
        """Documento `contracts/news_item.v1`."""
        return {
            "source": self.source,
            "feed": self.feed,
            "item_id": self.item_id,
            "title": self.title,
            "url": self.url,
            "published_at": _utc_z(self.published_at),
            "retrieved_at": _utc_z(self.retrieved_at),
        }


class _Element(Protocol):
    def findtext(self, path: str) -> str | None: ...


def _text(item: _Element, tag: str) -> str:
    return (item.findtext(tag) or "").strip()


def _published(raw: str, where: str) -> datetime:
    if not raw:
        raise FeedError(f"{where}: sin pubDate")
    try:
        t = parsedate_to_datetime(raw)
    except (TypeError, ValueError) as e:
        raise FeedError(f"{where}: pubDate inválido {raw!r}") from e
    if t.tzinfo is None:
        raise FeedError(f"{where}: pubDate sin zona horaria {raw!r}")
    return t.astimezone(UTC)


def parse_rss(xml: bytes, feed: FeedConfig, retrieved_at: datetime) -> list[NewsItem]:
    """Ítems de un RSS 2.0. Cualquier ítem que no cumple las reglas hace fallar el feed entero."""
    if retrieved_at.tzinfo is None:
        raise FeedError("retrieved_at tiene que tener zona horaria")
    try:
        root = ElementTree.fromstring(xml)
    except (ElementTree.ParseError, DefusedXmlException) as e:
        raise FeedError(f"{feed.source}/{feed.name}: XML inválido o peligroso") from e
    items: list[NewsItem] = []
    for i, item in enumerate(root.iter("item")):
        where = f"{feed.source}/{feed.name} ítem {i}"
        title = _text(item, "title")
        url = _text(item, "link")
        item_id = _text(item, "guid") or url
        if not 0 < len(title) <= _MAX_TEXT:
            raise FeedError(f"{where}: título vacío o demasiado largo")
        if not url.startswith("https://") or len(url) > _MAX_URL:
            raise FeedError(f"{where}: enlace inválido {url!r}")
        if len(item_id) > _MAX_TEXT:
            raise FeedError(f"{where}: id demasiado largo")
        published = _published(_text(item, "pubDate"), where)
        if published > retrieved_at:
            raise FeedError(f"{where}: publicado en el futuro ({published.isoformat()})")
        items.append(
            NewsItem(
                source=feed.source,
                feed=feed.name,
                item_id=item_id,
                title=title,
                url=url,
                published_at=published,
                retrieved_at=retrieved_at.astimezone(UTC),
            )
        )
    return items


Opener = Callable[[urllib.request.Request, int], bytes]


def fetch(feed: FeedConfig, cfg: NewsConfig, opener: Opener = https_get) -> list[NewsItem]:
    """Baja un feed y lo convierte. `opener` se reemplaza en los tests (sin red en CI)."""
    req = urllib.request.Request(feed.url, headers={"User-Agent": cfg.user_agent})  # noqa: S310
    body = opener(req, cfg.timeout_secs)
    return parse_rss(body, feed, datetime.now(UTC))
