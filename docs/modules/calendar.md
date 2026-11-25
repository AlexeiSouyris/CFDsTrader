---
modulo: calendar
ruta: `intelligence/cfds_intel/calendar`
lenguaje: Python
estado: hecho
resumen: Calendario económico de Forex Factory, solo por su feed JSON oficial (ADR 0010).
---

# calendar

## Responsabilidad (única)

- Calendario económico de Forex Factory **solo por su feed JSON oficial** (ADR 0010, sin scraping ni evasión de bots): mapea moneda → país con `config/calendar.toml` a `economic_event.v1`, descarta los feriados y guarda en `economic_events` (solo agregar, `published_at` = la primera vez que se vio). Comando `python -m cfds_intel.calendar`. Lo lee el filtro §7.2 del motor (#243)

## Estado

- hecho (#239)
