# Propuesta de riesgo: grupos correlacionados, gaps, caída gradual y volatilidad objetivo

- **Fecha:** 25-11-2026.
- **Issues:** #670 (punto 4 del plan, D1) y #671 (punto 9, D2), del autoanálisis `docs/analysis/2026-11-autoanalisis.md` (#658).
- **Estado:** la mecánica está en el módulo de riesgo (`engine/crates/risk`) con tests, pero **todo sale apagado** en `config/risk.toml`. Un test (`engine/crates/risk/tests/unchanged.rs`) prueba que, con la config del repositorio, `check` da exactamente lo mismo que antes, decimal por decimal. El runner de paper (#382, desde el 27-11) **no cambia** hasta que el dueño apruebe valores.
- **Los valores de abajo son recomendaciones, no decisiones.** Son parámetros de riesgo: los aprueba el dueño (CLAUDE.md, modo de un solo constructor). Nada de esto es consejo de inversión.

## Puntos para el dueño

| # | Parámetro (`config/risk.toml`) | Hoy (apagado) | Recomendado | Por qué | Estado |
|---|---|---|---|---|---|
| 1 | `[[correlation.groups]]` `us_equity_index` = US100, US500, US30, US2000 | sin grupos | `max_risk_pct = "1.5"` | Los cuatro índices de EE. UU. se mueven casi igual: hoy 5 posiciones al 1 % en ellos son casi una sola apuesta del 5 %. Con 1,5 % entran una entera y media; la segunda se achica | **PENDIENTE** |
| 2 | Otros grupos (DE40, XAUUSD) | — | ninguno por ahora | DE40 y el oro no tienen pareja en el catálogo; un grupo de uno es un error de config | **PENDIENTE** |
| 3 | `[gap] gap_atr` | `"0"` | `"0.25"` (provisional) | Un gap por noticias salta el stop. Con stops de 1 a 3 ATR, 0,25 ATR achica el tamaño entre 8 y 20 %, y `risk` pasa a contar ese salto. Conviene fijarlo con el deslizamiento real más allá del stop que mida el paper | **PENDIENTE** |

## Qué hace cada cosa

### 1. Tope por grupo correlacionado (#670)

- Cada grupo tiene un nombre, sus instrumentos (al menos 2, sin repetir, ids de `config/instruments.toml`) y un tope `max_risk_pct` en % del capital actual.
- **Riesgo abierto del grupo:** la suma de lo que todavía arriesgan sus posiciones abiertas hasta el stop, en EUR. El broker simulado usa el stop original y la fracción que sigue abierta; el backtest de cartera, el tamaño completo. Las dos son cotas conservadoras (si el stop ya se movió a favor, cuentan de más).
- **Lugar libre** = tope − riesgo abierto. Si el instrumento está en varios grupos, manda el más lleno.
  - Sin lugar (≤ 0): se rechaza con el motivo nuevo **`group_risk_limit`** (contratos `order_report.v1` y `ticket_verdict.v1`, texto en la UI).
  - Con lugar parcial: el tamaño **se achica** hasta llenar el lugar, igual que hace el tope de apalancamiento. `Sizing.group_capped` lo indica. Después siguen valiendo el tope de apalancamiento, el margen y el tamaño mínimo del broker.
- Un instrumento sin grupo no mira los grupos.
- El orden de los rechazos no cambia: kill switch, drawdown, pérdida diaria, cantidad de posiciones, instrumento abierto, distancia al stop y recién ahí el grupo.

### 2. Colchón por gaps (#670)

- `gap_atr` (≥ 0, en múltiplos del ATR) se suma a la distancia al stop **solo para calcular el tamaño**: tamaño = riesgo por operación / (distancia al stop + `gap_atr` × ATR).
- `Sizing.risk` pasa a ser tamaño × (distancia + colchón): lo que se perdería con un salto de ese tamaño por encima del stop. Los límites de distancia al stop (0,5 a 3 ATR) no cambian.
- Efecto en los informes: el backtest de investigación usa `Sizing.risk` como 1 R. Con un colchón mayor que 0, 1 R incluye el gap y los R por operación salen más chicos. Con 0, todo es igual.

## Qué sigue después de la aprobación

1. El dueño elige los valores (o los cambia) y lo deja escrito en #670.
2. Un PR aparte cambia `config/risk.toml`, actualiza el test de la config del repositorio y deja de exigir que el comportamiento sea igual al anterior.
3. Antes de prenderlo en paper: una corrida del backtest de cartera con y sin los topes, registrada en `docs/analysis/registro-de-resultados.csv`.
