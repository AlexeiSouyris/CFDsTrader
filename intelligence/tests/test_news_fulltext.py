"""Texto oficial de una noticia (ADR 0008 §4). Sin red."""

import re
import urllib.request
from collections.abc import Callable
from datetime import UTC, datetime

import pytest

from cfds_intel.news import NewsItem
from cfds_intel.news.fulltext import (
    GAP,
    FullTextError,
    Layout,
    fetch_text,
    follow_patterns,
    html_to_text,
    layout_from,
    select_text,
)

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


# --- Selección del texto (#190) ---------------------------------------------------------------


# Documento largo de juguete: inicio, relleno, tres secciones con relleno entre medio.
FILL = "x " * 200
DOC = (
    f"Inicio del comunicado. {FILL}"
    f"Staff Outlook staff dice cosas. {FILL}"
    f"Discussion miembros discuten. {FILL}"
    f"Policy Actions decidieron subir. {FILL}"
)
SECTIONS = tuple(re.compile(p) for p in ["Policy Actions", "Discussion", "Staff Outlook"])


def test_without_layout_it_is_the_beginning_as_before() -> None:
    assert select_text(DOC, 100, Layout()) == DOC[:100].strip()


def test_short_text_is_whole() -> None:
    lay = Layout(head_chars=10, sections=SECTIONS, section_max_chars=20)
    assert select_text("Inicio corto.", 100, lay) == "Inicio corto."


def test_takes_head_then_sections_by_priority_in_document_order() -> None:
    lay = Layout(head_chars=22, sections=SECTIONS, section_max_chars=32)
    # Lugar justo para el inicio, dos secciones y las marcas: la de menor prioridad no entra.
    out = select_text(DOC, 22 + 3 * len(GAP) + 32 + 29, lay)
    assert out.startswith("Inicio del comunicado.")
    assert "Discussion miembros discuten." in out and "Policy Actions decidieron subir" in out
    assert "Staff Outlook" not in out
    assert out.index("Discussion") < out.index("Policy Actions"), "orden del documento"
    assert GAP in out


def test_never_exceeds_max_chars() -> None:
    lay = Layout(head_chars=30, sections=SECTIONS, section_max_chars=500)
    for n in (60, 100, 333, 1000, len(DOC) - 1):
        assert len(select_text(DOC, n, lay)) <= n


def test_section_ends_at_the_next_listed_heading() -> None:
    text = "Inicio. " + FILL + "Discussion a b c. Policy Actions subir. " + FILL
    first_discussion = (re.compile("Discussion"), re.compile("Policy Actions"))
    lay = Layout(head_chars=8, sections=first_discussion, section_max_chars=1000)
    # Inicio + marcas + justo la sección "Discussion": no sigue de largo en "Policy Actions".
    out = select_text(text, 8 + 2 * len(GAP) + len("Discussion a b c. "), lay)
    assert out == f"Inicio.{GAP}Discussion a b c."


def test_leftover_budget_extends_the_head() -> None:
    lay = Layout(head_chars=10, sections=(re.compile("no existe"),), section_max_chars=50)
    assert select_text(DOC, 120, lay) == DOC[:120].strip()


def test_section_inside_the_head_is_not_taken_twice() -> None:
    text = "Policy Actions al principio. " + FILL
    lay = Layout(head_chars=40, sections=SECTIONS, section_max_chars=40)
    out = select_text(text, 100, lay)
    assert out.count("Policy Actions") == 1 and len(out) <= 100


def test_drop_removes_site_chrome_first() -> None:
    lay = Layout(drop=(re.compile(r"Skip to main content .*? Search Toggle Button", re.S),))
    text = "Título Skip to main content menú\nlargo Search Toggle Button Cuerpo."
    assert select_text(text, 100, lay) == "Título Cuerpo."


@pytest.mark.parametrize(
    ("table", "msg"),
    [
        ({"sections": "Policy"}, "text.sections"),
        ({"drop": ["(unclosed"]}, "text.drop"),
        ({"head_chars": -1}, "head_chars"),
        ({"head_chars": 7000}, "no puede pasar"),
        ({"sections": ["Policy"]}, "section_max_chars"),
        ({"scan_max_chars": 10}, "scan_max_chars"),
    ],
)
def test_layout_config_is_validated(table: dict[str, object], msg: str) -> None:
    with pytest.raises(FullTextError, match=msg):
        layout_from(table, 6000)


def test_linked_pdf_is_scanned_beyond_max_chars_to_find_sections() -> None:
    pdf_doc = "https://www.federalreserve.gov/monetarypolicy/files/fomcminutes20261105.pdf"
    _, fake = site(
        {
            RELEASE: release_page("/monetarypolicy/files/fomcminutes20261105.pdf"),
            pdf_doc: tiny_pdf("Policy Actions held"),
        }
    )
    lay = Layout(head_chars=5, sections=SECTIONS, section_max_chars=40, scan_max_chars=10_000)
    text = fetch_text(item(RELEASE), HOSTS, 40, "ua", 5, opener=fake, follow=FOLLOW, layout=lay)
    assert "Policy Actions held" in text and len(text) <= 40
