---
modulo: calendar
ruta: `intelligence/cfds_intel/calendar`
lenguaje: Python
estado: hecho
resumen: Calendario económico de Forex Factory, solo por su feed JSON oficial (ADR 0010), con su forecast y previous (ADR 0018).
---

# calendar

## Responsabilidad (única)

- Calendario económico de Forex Factory **solo por su feed JSON oficial** (ADR 0010, sin scraping ni evasión de bots): mapea moneda → país con `config/calendar.toml` a `economic_event.v1`, descarta los feriados y guarda en `economic_events` (solo agregar, `published_at` = la primera vez que se vio). Comando `python -m cfds_intel.calendar`. Lo lee el filtro §7.2 del motor (#243)
- En la misma lectura, sin pedidos extra (ADR 0018 §1, #696): el `forecast` y el `previous` de cada evento guardado como instantáneas en `calendar_estimates` (`calendar_estimate.v1`): texto tal cual, número `Decimal` y unidad solo si no hay ambigüedad (`estimates.py`), fila nueva solo cuando el par cambia, con `seen_at`. La sorpresa no se calcula acá: la calcula Rust (#698)

## Estado

- hecho (#239); forecast y previous (#696)
