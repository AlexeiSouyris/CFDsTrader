"""Ingesta de noticias oficiales (#109, ADR 0006), con feeds sintéticos y sin red."""

import json
import urllib.request
from datetime import UTC, datetime
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

from cfds_intel.news import FeedConfig, FeedError, fetch, load_config, parse_rss

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = (Path(__file__).parent / "fixtures" / "rss_fed_synthetic.xml").read_bytes()
FEED = FeedConfig(
    "fed", "press_monetary", "https://www.federalreserve.gov/feeds/press_monetary.xml"
)
NOW = datetime(2026, 11, 21, 3, 0, tzinfo=UTC)


def schema() -> Draft202012Validator:
    doc = json.loads((ROOT / "contracts" / "news_item.v1.schema.json").read_text())
    return Draft202012Validator(doc)


def rss(item: str) -> bytes:
    return f'<rss version="2.0"><channel>{item}</channel></rss>'.encode()


def item(
    title: str = "Título",
    link: str = "https://www.ecb.europa.eu/press/x.html",
    pub: str = "Thu, 19 Nov 2026 13:45:00 +0100",
) -> str:
    return f"<item><title>{title}</title><link>{link}</link><pubDate>{pub}</pubDate></item>"


def test_items_follow_the_contract_with_the_real_publication_time_in_utc() -> None:
    items = parse_rss(FIXTURE, FEED, NOW)
    assert [i.title for i in items] == ["Comunicado de prueba A", "Comunicado de prueba B"]
    docs = [i.to_contract() for i in items]
    for d in docs:
        assert list(schema().iter_errors(d)) == []
    assert docs[0]["published_at"] == "2026-11-18T19:00:00Z"
    # -0400 → UTC.
    assert docs[1]["published_at"] == "2026-10-28T18:00:00Z"
    assert docs[0]["retrieved_at"] == "2026-11-21T03:00:00Z"
    # El id es el guid; sin guid, el enlace.
    assert docs[0]["item_id"].endswith("prueba-a.htm")
    assert docs[1]["item_id"] == docs[1]["url"]


@pytest.mark.parametrize(
    "bad",
    [
        item(pub=""),  # sin fecha: no se inventa
        item(pub="ayer a la tarde"),
        item(pub="Thu, 19 Nov 2026 13:45:00"),  # sin zona horaria
        item(pub="Sat, 21 Nov 2026 04:00:00 GMT"),  # posterior a la lectura
        item(link="http://www.ecb.europa.eu/x"),  # no https
        item(title=""),
        item(title="x" * 501),
    ],
)
def test_a_bad_item_rejects_the_whole_feed(bad: str) -> None:
    with pytest.raises(FeedError):
        parse_rss(rss(bad), FEED, NOW)


def test_malformed_or_dangerous_xml_is_rejected() -> None:
    with pytest.raises(FeedError):
        parse_rss(b"<rss><channel><item>", FEED, NOW)
    bomb = (
        b'<?xml version="1.0"?><!DOCTYPE r [<!ENTITY a "aaaa"><!ENTITY b "&a;&a;&a;&a;">]>'
        b"<rss><channel><item><title>&b;</title></item></channel></rss>"
    )
    with pytest.raises(FeedError):
        parse_rss(bomb, FEED, NOW)


def test_retrieved_at_needs_a_time_zone() -> None:
    with pytest.raises(FeedError):
        parse_rss(FIXTURE, FEED, datetime(2026, 11, 21, 3, 0))


def test_the_contract_rejects_what_the_reader_never_produces() -> None:
    good = parse_rss(FIXTURE, FEED, NOW)[0].to_contract()
    v = schema()
    for key, value in [
        ("published_at", "2026-11-18T20:00:00+01:00"),
        ("url", "http://x"),
        ("source", "Fed"),
        ("title", ""),
    ]:
        assert not v.is_valid({**good, key: value}), key
    assert not v.is_valid({**good, "body": "texto completo"})


def test_repository_config_lists_the_approved_sources_only() -> None:
    cfg = load_config((ROOT / "config" / "news.toml").read_text())
    assert {f.source for f in cfg.feeds} == {"fed", "ecb"}
    assert all(f.url.startswith("https://") for f in cfg.feeds)
    assert "cfds" in cfg.user_agent and "@" not in cfg.user_agent, "sin datos personales"


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ('source = "fed"', 'source = "Fed"'),
        ('name = "press_monetary"', 'name = "press monetary"'),
        ("https://www.federalreserve.gov/feeds/press_monetary.xml", "http://x"),
        ("timeout_secs = 30", "timeout_secs = 0"),
        ('name = "press_all"', 'name = "press_monetary"'),  # repetido
    ],
)
def test_invalid_config_is_rejected(old: str, new: str) -> None:
    text = (ROOT / "config" / "news.toml").read_text()
    assert old in text
    with pytest.raises(FeedError):
        load_config(text.replace(old, new, 1))


def test_fetch_sends_the_configured_user_agent_and_never_touches_the_network() -> None:
    cfg = load_config((ROOT / "config" / "news.toml").read_text())
    seen: list[tuple[str, str | None, int]] = []

    def fake(req: urllib.request.Request, timeout: int) -> bytes:
        seen.append((req.full_url, req.get_header("User-agent"), timeout))
        return FIXTURE

    items = fetch(cfg.feeds[0], cfg, opener=fake)
    assert len(items) == 2
    assert seen == [(cfg.feeds[0].url, cfg.user_agent, cfg.timeout_secs)]
