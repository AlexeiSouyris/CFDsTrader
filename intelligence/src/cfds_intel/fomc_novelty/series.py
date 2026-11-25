"""Serie de novedad: cada comunicado contra el inmediatamente anterior (#668). Funciones puras.

El primer comunicado del período no tiene fila (no hay con qué compararlo). El valor de cada
fila solo existe desde la hora de su comunicado (`available_at`): usa ese texto y el anterior,
nada posterior.
"""

from collections.abc import Callable, Sequence
from datetime import datetime
from decimal import Decimal
from itertools import pairwise

from cfds_intel.fomc_novelty.model import MeasuredText, NoveltyError, NoveltyRow
from cfds_intel.fomc_novelty.text import word_change_share, words


def _check_order(texts: Sequence[MeasuredText]) -> None:
    for prev, cur in pairwise(texts):
        if not prev.statement.scheduled_at < cur.statement.scheduled_at:
            raise NoveltyError(
                f"comunicados fuera de orden o repetidos: {prev.statement.reference_period}"
                f" y {cur.statement.reference_period}"
            )


def _row(
    prev: MeasuredText,
    cur: MeasuredText,
    method: str,
    version: str,
    model: str | None,
    digest: str | None,
    value: Decimal,
    computed_at: datetime,
) -> NoveltyRow:
    return NoveltyRow(
        reference_period=cur.statement.reference_period,
        status=cur.statement.status,
        statement_url=cur.statement.source_url,
        available_at=cur.statement.scheduled_at,
        previous_reference_period=prev.statement.reference_period,
        previous_statement_url=prev.statement.source_url,
        method=method,
        method_version=version,
        model=model,
        model_digest=digest,
        value=value,
        text_sha256=cur.sha256,
        previous_text_sha256=prev.sha256,
        word_count=cur.word_count,
        previous_word_count=prev.word_count,
        computed_at=computed_at,
    )


def word_diff_rows(
    texts: Sequence[MeasuredText], version: str, computed_at: datetime
) -> list[NoveltyRow]:
    _check_order(texts)
    return [
        _row(
            prev,
            cur,
            "word_diff",
            version,
            None,
            None,
            word_change_share(words(prev.text), words(cur.text)),
            computed_at,
        )
        for prev, cur in pairwise(texts)
    ]


def embedding_rows(
    texts: Sequence[MeasuredText],
    vector_of: Callable[[str], list[float]],
    distance: Callable[[list[float], list[float]], Decimal],
    model: str,
    digest: str,
    version: str,
    computed_at: datetime,
) -> list[NoveltyRow]:
    _check_order(texts)
    vectors = [vector_of(t.text) for t in texts]
    return [
        _row(prev, cur, "embedding_cosine", version, model, digest, distance(a, b), computed_at)
        for (prev, a), (cur, b) in pairwise(zip(texts, vectors, strict=True))
    ]
