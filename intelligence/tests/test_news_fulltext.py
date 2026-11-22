"""Texto oficial de una noticia (ADR 0008 §4). Sin red."""

import urllib.request
from datetime import UTC, datetime

import pytest

from cfds_intel.news import NewsItem
from cfds_intel.news.fulltext import FullTextError, fetch_text, html_to_text

HOSTS = {"www.federalreserve.gov", "www.ecb.europa.eu"}
AT = datetime(2026, 11, 20, 19, 0, tzinfo=UTC)


def item(url: str) -> NewsItem:
    return NewsItem("fed", "press_monetary", url, "FOMC statement", url, AT, AT)


PAGE = b"""<html><head><style>.x{}</style><script>var a=1;</script></head>
<body><nav>Inicio | Noticias</nav><h1>FOMC statement</h1>
<p>The Committee decided to raise the target range &amp; will monitor.</p>
<footer>Last update</footer></body></html>"""


def test_visible_text_without_scripts_menus_or_footer() -> None:
    text = html_to_text(PAGE.decode())
    assert text == "FOMC statement The Committee decided to raise the target range & will monitor."


def test_fetch_uses_the_shared_https_opener_and_trims() -> None:
    seen: dict[str, str] = {}

    def fake(req: urllib.request.Request, timeout: int) -> bytes:
        seen["url"], seen["ua"] = req.full_url, req.get_header("User-agent") or ""
        return PAGE

    url = "https://www.federalreserve.gov/newsevents/pressreleases/monetary20261120a.htm"
    text = fetch_text(item(url), HOSTS, 20, "cfds-test", 5, opener=fake)
    assert text == "FOMC statement The C"
    assert seen == {"url": url, "ua": "cfds-test"}


def test_other_sites_and_failures_are_errors() -> None:
    def never(req: urllib.request.Request, timeout: int) -> bytes:
        raise AssertionError("no tenía que pedir nada")

    with pytest.raises(FullTextError, match="allowed_hosts"):
        fetch_text(item("https://evil.example.com/x"), HOSTS, 100, "ua", 5, opener=never)

    def down(req: urllib.request.Request, timeout: int) -> bytes:
        raise OSError("timeout")

    url = "https://www.ecb.europa.eu/press/x.html"
    with pytest.raises(FullTextError, match="no se pudo leer"):
        fetch_text(item(url), HOSTS, 100, "ua", 5, opener=down)
    with pytest.raises(FullTextError, match="no tiene texto"):
        fetch_text(item(url), HOSTS, 100, "ua", 5, opener=lambda r, t: b"<script>x</script>")
