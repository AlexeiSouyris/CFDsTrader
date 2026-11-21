"""Noticias de fuentes oficiales (ADR 0006): ingesta con hora de publicación real."""

from cfds_intel.news.feeds import (
    FeedConfig,
    FeedError,
    NewsConfig,
    NewsItem,
    fetch,
    load_config,
    parse_rss,
)

__all__ = [
    "FeedConfig",
    "FeedError",
    "NewsConfig",
    "NewsItem",
    "fetch",
    "load_config",
    "parse_rss",
]
