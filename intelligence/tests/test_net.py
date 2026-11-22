"""Salida HTTPS con el almacén de certificados del sistema (#121). Sin red."""

import inspect
import ssl
import urllib.request
from typing import Any

import pytest
import truststore

from cfds_intel import net
from cfds_intel.news import feeds
from cfds_intel.sentiment import cot


def test_context_uses_system_store_and_always_verifies() -> None:
    ctx = net.tls_context()
    assert isinstance(ctx, truststore.SSLContext)
    assert ctx.verify_mode == ssl.CERT_REQUIRED
    assert ctx.check_hostname is True
    assert ctx.minimum_version >= ssl.TLSVersion.TLSv1_2


def test_global_ssl_is_not_patched() -> None:
    net.tls_context()
    assert ssl.SSLContext is not truststore.SSLContext


class _FakeResponse:
    def __enter__(self) -> "_FakeResponse":
        return self

    def __exit__(self, *_: object) -> None:
        return None

    def read(self) -> bytes:
        return b"ok"


def test_https_get_passes_the_system_context(monkeypatch: pytest.MonkeyPatch) -> None:
    seen: dict[str, Any] = {}

    def fake_urlopen(req: urllib.request.Request, **kwargs: Any) -> _FakeResponse:
        seen["url"] = req.full_url
        seen.update(kwargs)
        return _FakeResponse()

    monkeypatch.setattr(urllib.request, "urlopen", fake_urlopen)
    req = urllib.request.Request("https://www.ecb.europa.eu/rss/press.html")
    assert net.https_get(req, 7) == b"ok"
    assert seen["url"] == "https://www.ecb.europa.eu/rss/press.html"
    assert seen["timeout"] == 7
    assert seen["context"] is net.tls_context()


@pytest.mark.parametrize(
    "url", ["http://www.ecb.europa.eu/rss/press.html", "file:///etc/passwd", "ftp://x/y"]
)
def test_https_get_rejects_other_schemes(monkeypatch: pytest.MonkeyPatch, url: str) -> None:
    def no_network(*_: object, **__: object) -> None:
        raise AssertionError("no tenía que abrir una conexión")

    monkeypatch.setattr(urllib.request, "urlopen", no_network)
    with pytest.raises(net.InsecureUrlError):
        net.https_get(urllib.request.Request(url), 5)  # noqa: S310 (el test es que se rechace)


def test_news_and_cot_use_the_shared_opener() -> None:
    assert inspect.signature(feeds.fetch).parameters["opener"].default is net.https_get
    assert inspect.signature(cot.fetch_rows).parameters["opener"].default is net.https_get
