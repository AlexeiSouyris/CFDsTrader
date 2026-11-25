"""Embeddings locales con Ollama y distancia coseno (#668).

- Solo `http://127.0.0.1` (ADR 0008), con `local_http` de `cfds_intel.news.ollama`.
- El modelo se fija por nombre y por el digest de `GET /api/tags`, que va en cada fila.
- `POST /api/embed` con `truncate: false`: si el texto no entra en el contexto, falla en vez de
  recortarlo en silencio.
- No se le pide opinión al modelo: solo un vector del texto. No conoce precios ni resultados.
"""

import math
from decimal import ROUND_HALF_EVEN, Decimal

from cfds_intel.fomc_novelty.model import EmbeddingConfig, NoveltyError
from cfds_intel.news.ollama import Transport, local_http

SIX = Decimal("0.000001")


def model_digest(cfg: EmbeddingConfig, transport: Transport = local_http) -> str:
    tags = transport("GET", f"{cfg.url}/api/tags", None, cfg.timeout_secs)
    for m in tags.get("models", []):
        if cfg.model in (m.get("name"), m.get("model"), f"{m.get('name', '')}".split(":")[0]):
            digest = m.get("digest")
            if isinstance(digest, str) and digest:
                return digest
    raise NoveltyError(
        f"el modelo {cfg.model!r} no está instalado en Ollama: el dueño lo instala con"
        f" `ollama pull {cfg.model}`"
    )


def embed(cfg: EmbeddingConfig, text: str, transport: Transport = local_http) -> list[float]:
    body = {
        "model": cfg.model,
        "input": cfg.prefix + text,
        "truncate": False,
        "options": {"num_ctx": cfg.num_ctx},
    }
    out = transport("POST", f"{cfg.url}/api/embed", body, cfg.timeout_secs)
    vectors = out.get("embeddings")
    if (
        not isinstance(vectors, list)
        or len(vectors) != 1
        or not isinstance(vectors[0], list)
        or not vectors[0]
        or not all(isinstance(x, int | float) for x in vectors[0])
    ):
        raise NoveltyError("Ollama no devolvió un embedding")
    return [float(x) for x in vectors[0]]


def cosine_distance(a: list[float], b: list[float]) -> Decimal:
    """1 - coseno, entre 0 y 2, redondeada a 6 decimales."""
    if len(a) != len(b) or not a:
        raise NoveltyError("embeddings de distinto largo")
    na = math.sqrt(math.fsum(x * x for x in a))
    nb = math.sqrt(math.fsum(x * x for x in b))
    if na == 0 or nb == 0:
        raise NoveltyError("embedding nulo")
    cos = math.fsum(x * y for x, y in zip(a, b, strict=True)) / (na * nb)
    cos = max(-1.0, min(1.0, cos))
    return Decimal(repr(1.0 - cos)).quantize(SIX, rounding=ROUND_HALF_EVEN).max(Decimal(0))
