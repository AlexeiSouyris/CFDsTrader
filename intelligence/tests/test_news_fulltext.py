"""Texto oficial de una noticia (ADR 0008 §4). Sin red."""

import urllib.request
from collections.abc import Callable
from datetime import UTC, datetime

import pytest

from cfds_intel.news import NewsItem
from cfds_intel.news.fulltext import FullTextError, fetch_text, follow_patterns, html_to_text

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


# --- documentos enlazados (#168) ---

FOLLOW = follow_patterns(
    {
        "follow": [
            r"^/monetarypolicy/fomcminutes\d{8}\.htm$",
            r"^/monetarypolicy/files/fomcminutes\d{8}\.pdf$",
        ]
    }
)
RELEASE = "https://www.federalreserve.gov/newsevents/pressreleases/monetary20261120a.htm"


def release_page(*hrefs: str) -> bytes:
    links = "".join(f'<a href="{h}">doc</a> ' for h in hrefs)
    return f"<html><body><p>Minutes released.</p>{links}</body></html>".encode()


def tiny_pdf(text: str) -> bytes:
    """PDF mínimo de una página con `text`, armado a mano (sin red ni archivos)."""
    stream = f"BT /F1 12 Tf 72 720 Td ({text}) Tj ET".encode()
    objs = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
        b"/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]
    out = bytearray(b"%PDF-1.4\n")
    offsets = []
    for i, o in enumerate(objs, 1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % i + o + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)
    out += b"".join(b"%010d 00000 n \n" % off for off in offsets)
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, xref)
    return bytes(out)


def site(
    pages: dict[str, bytes],
) -> tuple[list[str], Callable[[urllib.request.Request, int], bytes]]:
    seen: list[str] = []

    def fake(req: urllib.request.Request, timeout: int) -> bytes:
        seen.append(req.full_url)
        if req.full_url not in pages:
            raise OSError("404")
        return pages[req.full_url]

    return seen, fake


def test_follows_the_linked_html_minutes() -> None:
    doc = "https://www.federalreserve.gov/monetarypolicy/fomcminutes20261105.htm"
    seen, fake = site(
        {
            RELEASE: release_page("/monetarypolicy/fomcminutes20261105.htm"),
            doc: b"<p>Participants agreed inflation remained elevated.</p>",
        }
    )
    text = fetch_text(item(RELEASE), HOSTS, 500, "ua", 5, opener=fake, follow=FOLLOW)
    assert text == "Minutes released. doc Participants agreed inflation remained elevated."
    assert seen == [RELEASE, doc]


def test_prefers_html_over_pdf_by_pattern_order() -> None:
    html_doc = "https://www.federalreserve.gov/monetarypolicy/fomcminutes20261105.htm"
    pdf_doc = "https://www.federalreserve.gov/monetarypolicy/files/fomcminutes20261105.pdf"
    page = release_page(
        "/monetarypolicy/files/fomcminutes20261105.pdf", "/monetarypolicy/fomcminutes20261105.htm"
    )
    seen, fake = site({RELEASE: page, html_doc: b"<p>html</p>", pdf_doc: tiny_pdf("pdf")})
    fetch_text(item(RELEASE), HOSTS, 500, "ua", 5, opener=fake, follow=FOLLOW)
    assert seen == [RELEASE, html_doc]


def test_reads_a_linked_pdf_in_memory() -> None:
    pdf_doc = "https://www.federalreserve.gov/monetarypolicy/files/fomcminutes20261105.pdf"
    _, fake = site(
        {
            RELEASE: release_page("/monetarypolicy/files/fomcminutes20261105.pdf"),
            pdf_doc: tiny_pdf("Rates held steady"),
        }
    )
    text = fetch_text(item(RELEASE), HOSTS, 500, "ua", 5, opener=fake, follow=FOLLOW)
    assert text.endswith("Rates held steady")


@pytest.mark.parametrize(
    "href",
    [
        "http://www.federalreserve.gov/monetarypolicy/fomcminutes20261105.htm",
        "https://evil.example.com/monetarypolicy/fomcminutes20261105.htm",
        "/monetarypolicy/other20261105.htm",
    ],
)
def test_ignores_links_off_host_insecure_or_not_matching(href: str) -> None:
    seen, fake = site({RELEASE: release_page(href)})
    text = fetch_text(item(RELEASE), HOSTS, 500, "ua", 5, opener=fake, follow=FOLLOW)
    assert text == "Minutes released. doc"
    assert seen == [RELEASE]


def test_without_follow_no_link_is_followed() -> None:
    seen, fake = site({RELEASE: release_page("/monetarypolicy/fomcminutes20261105.htm")})
    fetch_text(item(RELEASE), HOSTS, 500, "ua", 5, opener=fake)
    assert seen == [RELEASE]


def test_unreadable_document_means_no_interpretation() -> None:
    _, fake = site({RELEASE: release_page("/monetarypolicy/fomcminutes20261105.htm")})
    with pytest.raises(FullTextError, match="no se pudo leer"):
        fetch_text(item(RELEASE), HOSTS, 500, "ua", 5, opener=fake, follow=FOLLOW)


def test_broken_pdf_is_an_error() -> None:
    pdf_doc = "https://www.federalreserve.gov/monetarypolicy/files/fomcminutes20261105.pdf"
    _, fake = site(
        {
            RELEASE: release_page("/monetarypolicy/files/fomcminutes20261105.pdf"),
            pdf_doc: b"%PDF-1.4 not really",
        }
    )
    with pytest.raises(FullTextError):
        fetch_text(item(RELEASE), HOSTS, 500, "ua", 5, opener=fake, follow=FOLLOW)


@pytest.mark.parametrize("bad", [["no-anchor"], "^/x", ["^/(unclosed"]])
def test_follow_config_is_validated(bad: object) -> None:
    with pytest.raises(FullTextError, match=r"text\.follow"):
        follow_patterns({"follow": bad})


def test_shipped_config_patterns_are_valid() -> None:
    import tomllib
    from pathlib import Path

    cfg = tomllib.loads(
        (Path(__file__).parents[2] / "config" / "news_interpreter.toml").read_text()
    )
    assert follow_patterns(cfg["text"])
