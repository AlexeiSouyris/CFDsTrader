"""Fila de `contracts/official_release.v1` y configuración del calendario oficial (ADR 0016).

La fila es genérica (FOMC, CPI y empleo del BLS) y se valida al crearse con las
mismas reglas que el contrato y la tabla `official_releases`: así un error se ve antes de
pedir la base.
"""

import re
import tomllib
from dataclasses import dataclass
from datetime import UTC, date, datetime, time
from typing import Any
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

SOURCES = {"fed_fomc": {"fomc_statement"}, "bls": {"cpi", "employment_situation"}}
STATUSES = {"scheduled", "rescheduled", "cancelled", "unscheduled"}
RELEASE_PAGE = "release_page"
_RULE_NAME = re.compile(r"^[a-z][a-z0-9_]{0,63}$")
_DAY = re.compile(r"^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$")
_MONTH = re.compile(r"^\d{4}-(0[1-9]|1[0-2])$")
_HHMM = re.compile(r"^([01]\d|2[0-3]):([0-5]\d)$")


class OfficialCalendarError(ValueError):
    """La fuente, la configuración o una fila no cumplen las reglas: no se guarda nada."""


def utc_z(t: datetime) -> str:
    return t.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


@dataclass(frozen=True)
class OfficialRelease:
    """Una fila de `official_release.v1`."""

    source: str
    series: str
    reference_period: str
    status: str
    scheduled_at: datetime | None
    published_at: datetime
    time_rule: str
    with_press_conference: bool | None
    source_url: str

    def __post_init__(self) -> None:
        where = f"{self.source}/{self.series} {self.reference_period}"
        if self.series not in SOURCES.get(self.source, set()):
            raise OfficialCalendarError(f"{where}: serie {self.series!r} no es de {self.source!r}")
        fed = self.source == "fed_fomc"
        if not (_DAY if fed else _MONTH).match(self.reference_period):
            raise OfficialCalendarError(f"{where}: reference_period inválido")
        if self.status not in STATUSES:
            raise OfficialCalendarError(f"{where}: status inválido {self.status!r}")
        if (self.status == "cancelled") != (self.scheduled_at is None):
            raise OfficialCalendarError(f"{where}: scheduled_at es null solo si se canceló")
        for t in (self.scheduled_at, self.published_at):
            if t is not None and t.tzinfo is None:
                raise OfficialCalendarError(f"{where}: hora sin zona horaria")
        if self.scheduled_at is not None and self.published_at > self.scheduled_at:
            raise OfficialCalendarError(f"{where}: published_at posterior a scheduled_at")
        if self.status == "unscheduled" and self.published_at != self.scheduled_at:
            raise OfficialCalendarError(f"{where}: no programada: published_at = scheduled_at")
        if not _RULE_NAME.match(self.time_rule):
            raise OfficialCalendarError(f"{where}: time_rule inválido {self.time_rule!r}")
        if fed != (self.with_press_conference is not None):
            raise OfficialCalendarError(f"{where}: with_press_conference es solo del FOMC")
        if not self.source_url.startswith("https://") or len(self.source_url) > 500:
            raise OfficialCalendarError(f"{where}: source_url tiene que ser https")

    def key(self) -> tuple[str, str, str, str, datetime | None]:
        """Clave natural de la tabla (ADR 0016 §5)."""
        return (self.source, self.series, self.reference_period, self.status, self.scheduled_at)

    def to_contract(self) -> dict[str, Any]:
        return {
            "source": self.source,
            "series": self.series,
            "reference_period": self.reference_period,
            "status": self.status,
            "scheduled_at": None if self.scheduled_at is None else utc_z(self.scheduled_at),
            "published_at": utc_z(self.published_at),
            "time_rule": self.time_rule,
            "with_press_conference": self.with_press_conference,
            "source_url": self.source_url,
        }


@dataclass(frozen=True)
class TimeRule:
    """Hora de un comunicado programado del FOMC sin hora en su página (ADR 0016 §3.2)."""

    name: str
    start: date  # incluido
    end: date  # excluido
    press_conference: bool | None
    at: time

    def applies(self, day: date, press_conference: bool) -> bool:
        return self.start <= day < self.end and self.press_conference in (None, press_conference)


@dataclass(frozen=True)
class Announcement:
    """Comunicado del calendario tentativo (o su revisión) y los años completos que trae."""

    url: str
    years: tuple[int, ...]


@dataclass(frozen=True)
class FedConfig:
    first_year: int
    last_year: int
    historical_url: str
    historical_last_year: int
    calendars_url: str
    release_url_prefix: str
    scheduled_meetings_per_year: int
    max_unscheduled_lag_days: int
    tz: ZoneInfo
    time_rules: tuple[TimeRule, ...]
    announcements: tuple[Announcement, ...]

    def rule_for(self, day: date, press_conference: bool) -> TimeRule:
        found = [r for r in self.time_rules if r.applies(day, press_conference)]
        if len(found) != 1:
            names = [r.name for r in found]
            raise OfficialCalendarError(
                f"comunicado del {day}: tiene que valer una sola regla de hora y valen {names}"
            )
        return found[0]


@dataclass(frozen=True)
class Lapse:
    """Corte de fondos (lapse in appropriations): días sin publicaciones del BLS."""

    start: date  # primer día sin fondos
    end: date  # último día sin fondos


@dataclass(frozen=True)
class BlsNotice:
    """Página del BLS con las fechas revisadas por un corte de fondos (guardada a mano)."""

    url: str


@dataclass(frozen=True)
class BlsConfig:
    """CPI y empleo del BLS desde las páginas que el dueño guardó a mano (ADR 0016 §1.3)."""

    raw_dir: str
    first_year: int
    last_year: int
    year_url: str
    release_time: time
    time_rule: str
    notice_time_rule: str
    tz: ZoneInfo
    notices: tuple[BlsNotice, ...]
    lapses: tuple[Lapse, ...]


@dataclass(frozen=True)
class OfficialCalendarConfig:
    user_agent: str
    timeout_secs: int
    pause_secs: float
    fed: FedConfig
    bls: BlsConfig | None = None


def _int(raw: dict[str, Any], key: str, where: str) -> int:
    v = raw.get(key)
    if not isinstance(v, int) or isinstance(v, bool):
        raise OfficialCalendarError(f"{where}.{key} tiene que ser un entero")
    return v


def _https(raw: dict[str, Any], key: str, where: str) -> str:
    v = raw.get(key)
    if not (isinstance(v, str) and v.startswith("https://")):
        raise OfficialCalendarError(f"{where}.{key} tiene que ser https: {v!r}")
    return v


def _rule(raw: Any, i: int) -> TimeRule:
    where = f"fed.time_rules[{i}]"
    if not isinstance(raw, dict):
        raise OfficialCalendarError(f"{where} no es una tabla")
    unknown = set(raw) - {"name", "from", "to", "press_conference", "time", "source"}
    if unknown:
        raise OfficialCalendarError(f"{where}: claves desconocidas {sorted(unknown)}")
    name, at, src = raw.get("name"), raw.get("time"), raw.get("source")
    if not (isinstance(name, str) and _RULE_NAME.match(name)) or name == RELEASE_PAGE:
        raise OfficialCalendarError(f"{where}.name inválido: {name!r}")
    if not (isinstance(at, str) and _HHMM.match(at)):
        raise OfficialCalendarError(f"{where}.time tiene que ser HH:MM: {at!r}")
    if not (isinstance(src, str) and src.strip()):
        raise OfficialCalendarError(f"{where}.source: toda regla cita su fuente")
    try:
        start, end = date.fromisoformat(raw["from"]), date.fromisoformat(raw["to"])
    except (KeyError, TypeError, ValueError) as e:
        raise OfficialCalendarError(f"{where}: from y to tienen que ser AAAA-MM-DD") from e
    if start >= end:
        raise OfficialCalendarError(f"{where}: from tiene que ser anterior a to")
    pc = raw.get("press_conference")
    if pc is not None and not isinstance(pc, bool):
        raise OfficialCalendarError(f"{where}.press_conference tiene que ser true o false")
    return TimeRule(name, start, end, pc, time.fromisoformat(at))


def _announcement(raw: Any, i: int, prefix: str) -> Announcement:
    where = f"fed.schedule_announcements[{i}]"
    if not isinstance(raw, dict) or set(raw) != {"url", "years"}:
        raise OfficialCalendarError(f"{where}: tiene que tener url y years")
    url, years = raw["url"], raw["years"]
    if not (isinstance(url, str) and url.startswith(prefix)):
        raise OfficialCalendarError(f"{where}.url tiene que empezar con {prefix!r}: {url!r}")
    if not (
        isinstance(years, list)
        and years
        and all(isinstance(y, int) and not isinstance(y, bool) for y in years)
    ):
        raise OfficialCalendarError(f"{where}.years tiene que ser una lista de años")
    return Announcement(url, tuple(years))


def _bls(raw: Any) -> BlsConfig:
    if not isinstance(raw, dict):
        raise OfficialCalendarError("[bls] no es una tabla")
    keys = {
        "raw_dir",
        "first_year",
        "last_year",
        "year_url",
        "release_time",
        "time_rule",
        "notice_time_rule",
        "timezone",
        "notices",
        "lapses",
    }
    unknown = set(raw) - keys
    if unknown:
        raise OfficialCalendarError(f"bls: claves desconocidas {sorted(unknown)}")
    raw_dir = raw.get("raw_dir")
    if not (isinstance(raw_dir, str) and raw_dir.strip()):
        raise OfficialCalendarError("bls.raw_dir tiene que ser una carpeta")
    first, last = _int(raw, "first_year", "bls"), _int(raw, "last_year", "bls")
    if not 2000 <= first <= last:
        raise OfficialCalendarError("bls: años inválidos")
    year_url = _https(raw, "year_url", "bls")
    if "{year}" not in year_url:
        raise OfficialCalendarError("bls.year_url tiene que llevar {year}")
    at = raw.get("release_time")
    if not (isinstance(at, str) and _HHMM.match(at)):
        raise OfficialCalendarError(f"bls.release_time tiene que ser HH:MM: {at!r}")
    rules: list[str] = []
    for key in ("time_rule", "notice_time_rule"):
        r = raw.get(key)
        if not (isinstance(r, str) and _RULE_NAME.match(r)) or r == RELEASE_PAGE:
            raise OfficialCalendarError(f"bls.{key}: nombre de regla inválido {r!r}")
        rules.append(r)
    try:
        tz = ZoneInfo(str(raw.get("timezone")))
    except (ZoneInfoNotFoundError, ValueError) as e:
        raise OfficialCalendarError(f"bls.timezone desconocida: {raw.get('timezone')!r}") from e
    notices, lapses = raw.get("notices"), raw.get("lapses")
    if not isinstance(notices, list) or not notices:
        raise OfficialCalendarError("faltan [[bls.notices]]")
    if not isinstance(lapses, list) or not lapses:
        raise OfficialCalendarError("faltan [[bls.lapses]]")
    parsed_notices = []
    for i, n in enumerate(notices):
        if not isinstance(n, dict) or set(n) != {"url"}:
            raise OfficialCalendarError(f"bls.notices[{i}]: tiene que tener solo url")
        parsed_notices.append(BlsNotice(_https(n, "url", f"bls.notices[{i}]")))
    parsed_lapses = []
    for i, lp in enumerate(lapses):
        where = f"bls.lapses[{i}]"
        if not isinstance(lp, dict) or set(lp) != {"start", "end", "source"}:
            raise OfficialCalendarError(f"{where}: tiene que tener start, end y source")
        if not (isinstance(lp["source"], str) and lp["source"].strip()):
            raise OfficialCalendarError(f"{where}.source: todo corte cita su fuente")
        try:
            start, end = date.fromisoformat(lp["start"]), date.fromisoformat(lp["end"])
        except (TypeError, ValueError) as e:
            raise OfficialCalendarError(f"{where}: start y end tienen que ser AAAA-MM-DD") from e
        if start > end:
            raise OfficialCalendarError(f"{where}: start posterior a end")
        parsed_lapses.append(Lapse(start, end))
    parsed_lapses.sort(key=lambda lp: lp.start)
    return BlsConfig(
        raw_dir=raw_dir,
        first_year=first,
        last_year=last,
        year_url=year_url,
        release_time=time.fromisoformat(at),
        time_rule=rules[0],
        notice_time_rule=rules[1],
        tz=tz,
        notices=tuple(parsed_notices),
        lapses=tuple(parsed_lapses),
    )


def load_config(text: str) -> OfficialCalendarConfig:
    """Lee `config/official_calendar.toml` y lo valida."""
    raw = tomllib.loads(text)
    unknown = set(raw) - {"client", "fed", "bls"}
    if unknown:
        raise OfficialCalendarError(f"secciones desconocidas: {sorted(unknown)}")
    client, fed = raw.get("client", {}), raw.get("fed", {})
    ua = client.get("user_agent")
    if not isinstance(ua, str) or not ua.strip():
        raise OfficialCalendarError("client.user_agent tiene que ser un texto")
    timeout = _int(client, "timeout_secs", "client")
    pause = client.get("pause_secs")
    if timeout <= 0:
        raise OfficialCalendarError("client.timeout_secs tiene que ser mayor que 0")
    # ADR 0016 §1.2: al menos 1 s entre pedidos.
    if not isinstance(pause, int | float) or isinstance(pause, bool) or pause < 1:
        raise OfficialCalendarError("client.pause_secs tiene que ser al menos 1")
    first, last = _int(fed, "first_year", "fed"), _int(fed, "last_year", "fed")
    hist_last = _int(fed, "historical_last_year", "fed")
    per_year = _int(fed, "scheduled_meetings_per_year", "fed")
    max_lag = _int(fed, "max_unscheduled_lag_days", "fed")
    if not 0 <= max_lag <= 31:
        raise OfficialCalendarError("fed.max_unscheduled_lag_days tiene que estar entre 0 y 31")
    if not 2000 <= first <= last or per_year <= 0:
        raise OfficialCalendarError("fed: años o reuniones por año inválidos")
    historical = _https(fed, "historical_url", "fed")
    if "{year}" not in historical:
        raise OfficialCalendarError("fed.historical_url tiene que llevar {year}")
    prefix = _https(fed, "release_url_prefix", "fed")
    try:
        tz = ZoneInfo(str(fed.get("timezone")))
    except (ZoneInfoNotFoundError, ValueError) as e:
        raise OfficialCalendarError(f"fed.timezone desconocida: {fed.get('timezone')!r}") from e
    rules = fed.get("time_rules")
    anns = fed.get("schedule_announcements")
    if not isinstance(rules, list) or not rules:
        raise OfficialCalendarError("faltan [[fed.time_rules]]")
    if not isinstance(anns, list) or not anns:
        raise OfficialCalendarError("faltan [[fed.schedule_announcements]]")
    return OfficialCalendarConfig(
        user_agent=ua,
        timeout_secs=timeout,
        pause_secs=float(pause),
        fed=FedConfig(
            first_year=first,
            last_year=last,
            historical_url=historical,
            historical_last_year=hist_last,
            calendars_url=_https(fed, "calendars_url", "fed"),
            release_url_prefix=prefix,
            scheduled_meetings_per_year=per_year,
            max_unscheduled_lag_days=max_lag,
            tz=tz,
            time_rules=tuple(_rule(r, i) for i, r in enumerate(rules)),
            announcements=tuple(_announcement(a, i, prefix) for i, a in enumerate(anns)),
        ),
        bls=_bls(raw["bls"]) if "bls" in raw else None,
    )
