"""Calendario histórico oficial (ADR 0016, #520): comunicados del FOMC de federalreserve.gov.

Aparte de Forex Factory (`cfds_intel.calendar`, ADR 0010): otra fuente, otro contrato
(`official_release.v1`) y otra tabla (`official_releases`). El CPI y el empleo del BLS se suman
acá, en la misma tabla, cuando el dueño elija la forma de acceso (ADR 0016 §1.3).
"""

from cfds_intel.calendar.official.fed import FedResult, load_fed
from cfds_intel.calendar.official.model import (
    OfficialCalendarConfig,
    OfficialCalendarError,
    OfficialRelease,
    load_config,
)

__all__ = [
    "FedResult",
    "OfficialCalendarConfig",
    "OfficialCalendarError",
    "OfficialRelease",
    "load_config",
    "load_fed",
]
