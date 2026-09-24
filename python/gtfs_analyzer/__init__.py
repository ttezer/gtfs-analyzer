"""Python interface to the Rust GTFS Analyzer validation engine."""

from __future__ import annotations

import datetime as _datetime
import json as _json
from pathlib import Path
from typing import Any, Mapping, Optional, Union

from .gtfs_analyzer import validate as _validate_native

__all__ = ["ValidationError", "validate_gtfs"]
__version__ = "0.14.0"


class ValidationError(RuntimeError):
    """Raised when the Rust pipeline cannot produce a validation report."""


def _parse_today(today: Union[int, str]) -> int:
    if isinstance(today, int):
        text = str(today)
    elif isinstance(today, str):
        text = today.replace("-", "")
    else:
        raise TypeError("today must be an integer, YYYY-MM-DD string, or None")

    if len(text) != 8 or not text.isdigit():
        raise ValueError("today must be YYYYMMDD or YYYY-MM-DD")
    try:
        _datetime.date(
            int(text[0:4]),
            int(text[4:6]),
            int(text[6:8]),
        )
    except ValueError as error:
        raise ValueError("today is not a valid calendar date") from error
    return int(text)


def validate_gtfs(
    feed: Union[str, Path, bytes],
    *,
    today: Optional[Union[int, str]] = None,
    config: Optional[Mapping[str, Any]] = None,
    include_name_index: bool = False,
) -> dict[str, Any]:
    """Validate a GTFS ZIP path or bytes and return the result as a dict."""
    if isinstance(feed, (str, Path)):
        feed_bytes = Path(feed).read_bytes()
    elif isinstance(feed, bytes):
        feed_bytes = feed
    else:
        raise TypeError("feed must be a ZIP path or bytes")

    if today is None:
        today_value = int(_datetime.date.today().strftime("%Y%m%d"))
    else:
        today_value = _parse_today(today)

    if config is None:
        config_json = "{}"
    elif isinstance(config, Mapping):
        config_json = _json.dumps(config, separators=(",", ":"))
    else:
        raise TypeError("config must be a mapping or None")

    envelope = _json.loads(
        _validate_native(feed_bytes, today_value, config_json, include_name_index)
    )
    if "Fatal" in envelope:
        fatal = envelope["Fatal"]
        raise ValidationError(f"{fatal.get('code', 'validation failed')}: {fatal['message']}")
    return envelope["Ok"]
