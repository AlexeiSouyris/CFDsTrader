---
modulo: calendar.official
ruta: `intelligence/cfds_intel/calendar/official`
lenguaje: Python
estado: hecho
resumen: Calendario histórico oficial (ADR 0016): comunicados del FOMC y publicaciones del BLS.
---

# calendar.official

## Responsabilidad (única)

- Calendario histórico **oficial** (ADR 0016), aparte del de Forex Factory: comunicados del FOMC 2011–2025 de federalreserve.gov (páginas por año, de calendarios y de cada comunicado; solo por `cfds_intel.net`, User-Agent genérico, un pedido por vez con pausa), con la hora de la página o la regla de `config/official_calendar.toml` pasada a UTC con `America/New_York`, y `published_at` desde el anuncio del calendario tentativo (o el propio comunicado si fue de emergencia). Contrato `official_release.v1`, tabla `official_releases` (solo agregar, migración 0046). CPI y empleo del BLS 2011–2026 (`bls`) **sin robot** (ADR 0016 §1.3): solo lee los HTML que el dueño guardó a mano (`[bls] raw_dir`, fuera de git; cada página reconocida por su contenido), 08:30 ET a UTC, `published_at` desde el informe anterior de la serie, reprogramaciones y cancelaciones de los avisos de cierre (2013, 2025, 2026), lo no documentado queda fuera y una estructura inesperada hace fallar la carga. Comando `python -m cfds_intel.calendar.official [--source fed|bls] [--dry-run]`

## Estado

- FOMC (#520)
- BLS (#608)
