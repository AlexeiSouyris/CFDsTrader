# Velas diarias de IG: mediciones (21-11-2026)

Medido en la cuenta demo con `GET /prices/{epic}` v3, `resolution=DAY`. La resolución está documentada en labs.ig.com/reference/prices-epic.html. Sirven para los análogos v1 y las medidas propias de sentimiento (ADR 0004 §7 y 0005 §4), y respaldan la regla de #103 y #102.

## Cuándo empieza el día

| Período | Inicio de la vela (UTC) | Hora local en Europa central |
|---|---|---|
| Noviembre de 2026 (DE40 y US100) | 23:00 | 00:00 (CET) |
| Julio de 2026 (DE40) | 22:00 | 00:00 (CEST) |
| 28-03 al 01-04-2022, la semana siguiente al cambio de horario | **23:00** | **01:00** (CEST) |
| Desde el 04-04-2022 | 22:00 | 00:00 (CEST) |
| 31-10-2022, después del cambio de octubre | 23:00 | 00:00 (CET) |

- **Conclusión:** el día de IG va de medianoche a medianoche en hora de **Europa central**, la zona de la cuenta.
- **Excepción medida:** en la semana siguiente al cambio de marzo de 2022 empezó una hora tarde.
- **Regla adoptada** (`config/ig.toml` y `market-data::daily_bounds`):
  - el inicio puede llegar hasta **60 min** después de la medianoche local;
  - el fin es el más tardío entre la medianoche siguiente y el inicio + 24 h.

  Así una vela nunca se da por cerrada antes de tiempo. Cualquier otra hora de inicio hace fallar la carga.

## Días vacíos

IG manda algunos **sábados** con todos los precios en `null`: por ejemplo 26-02-2022, 06-03-2022, 09-04-2022, 18-06-2022, 29-10-2022, 27-09-2025 y 06-12-2025, en DE40. No son velas: se descartan y se informan.

## Carga de 5 años (22-11-2021 → 20-11-2026)

| Índice | Velas diarias | Días con sesión sin vela de IG |
|---|---|---|
| DE40 | 1.313 | 1: **19-06-2026** (Xetra abrió). Avisado, **no** se rellena |
| US100 | 1.312 | 0 |
| US500 | pendiente (cupo semanal) | — |
| US30 | pendiente (cupo semanal) | — |

## Cupo

- **Costo:** unos 270 puntos por año y por índice. IG cobra por vela devuelta, no por día del calendario.
- **Dos intentos fallidos con DE40** gastaron unos 2.700 puntos antes de tener la regla y la política de días faltantes. Desde entonces se prueba primero con un año.

Los precios quedan solo en la base local: no se commitean, porque el repo es público.
