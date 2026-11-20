# Spread de los índices en la cuenta demo de IG (noviembre 2026)

Fuente `ig-demo-spread` de `config/instruments.toml` (#67).

## Método

- Contratos: CFD al contado de **1 € por punto** (`IX.D.*.IFE.IP`), la variante elegida en #67.
- Datos: `GET /prices/{epic}` v3, resolución `HOUR`, del **2026-11-09 al 2026-11-20** (10 días hábiles), cuenta demo.
- Muestra: `ask − bid` del **cierre** de cada vela de 1h. El de apertura, máximo y mínimo no se usa, porque bid y ask no tienen por qué ser del mismo instante.
- **Spread en sesión:** mediana de las velas de 1h que caben enteras en la sesión regular de la bolsa del subyacente (`config/sessions.toml`; XNYS 09:30–16:00 Nueva York y XETR 09:00–17:30 Berlín).
- Se reproduce con `cargo run -p cfds-ig --example ig_market_terms -- 2026-11-09T00:00:00Z 2026-11-21T00:00:00Z`. Gasta unos 950 puntos del cupo semanal de IG.

## Resultado

| Índice | Epic | Velas en sesión | Spread en sesión (demo) | Mínimo publicado (ig.com/es) | Valor en el catálogo |
|---|---|---|---|---|---|
| US100 | `IX.D.NASDAQ.IFE.IP` | 60 | 1,5 | 1,6 | **1,6** (publicado) |
| US500 | `IX.D.SPTRD.IFE.IP` | 60 | 0,9 | 0,8 | **0,9** (medido) |
| US30 | `IX.D.DOW.IFE.IP` | 60 | 3,7 | 3,4 | **3,7** (medido) |
| DE40 | `IX.D.DAX.IFE.IP` | 80 | 1,4 | 1,8 | **1,8** (publicado) |

**Regla (decisión de Alexei en #67):** se usa el **mayor** de los dos. En DE40 y US100, la demo da spreads por debajo del mínimo que IG publica para la cuenta real. Usar el medido subestimaría los costos del backtest.

## Mediana por hora (UTC)

IG aplica **spreads fijos por franja**: dentro de cada franja, la mediana es la misma hora a hora.

| Hora UTC | DE40 | US100 | US30 | US500 |
|---|---|---|---|---|
| 00–06 | 5,0 | 2,5 | 6,0 | 1,6 |
| 07 | 2,8 | 2,5 | 5,0 | 1,6 |
| 08–13 | 1,4 | 2,5 | 5,0 | 1,6 |
| 14–15 | 1,4 | 1,5 | 3,7 | 0,9 |
| 16–20 | 2,8 | 1,5 | 3,7 | 0,9 |
| 21–22 | 7,0 | 5,8 | 12,2 | 1,9 |
| 23 | 7,0 | 2,5 | 6,0 | 1,6 |

Noviembre de 2026 es horario de invierno: Nueva York está en UTC−5 y Berlín en UTC+1. Fuera de la sesión regular, el spread es entre 1,5 y 5 veces mayor.

## Limitaciones

- La cuenta demo **puede no reflejar** los spreads de la cuenta real. Por eso se usa la regla del mayor.
- Diez días de muestra no incluyen eventos de volatilidad extrema, en los que IG puede ampliar el spread.
- Hay que volver a medirlo antes de la fase 5 (ADR 0002, consecuencias).
