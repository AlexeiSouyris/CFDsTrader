# Propuesta de riesgo: grupos correlacionados, gaps, caída gradual y volatilidad objetivo

- **Fecha:** 25-11-2026.
- **Issues:** #670 (punto 4 del plan, D1) y #671 (punto 9, D2), del autoanálisis `docs/analysis/2026-11-autoanalisis.md` (#658).
- **Estado (actualizado el 25-11-2026, #689):** el dueño aprobó los puntos 1 a 4 el 25-11-2026, textual: «1, 2 y 7 apruebo la recomendacion.» (registrado en https://github.com/AlexeiSouyris/CFDsTrader/issues/671#issuecomment-7169989627). Están en `config/risk.toml` desde #689 y el runner de paper (#382, desde el 27-11) los usa. La volatilidad objetivo (punto 5) sigue apagada. Un test (`engine/crates/risk/tests/unchanged.rs`) prueba que, con una config neutral (todo apagado), `check` da exactamente lo mismo que antes, decimal por decimal.
- **Los valores eran recomendaciones; las decisiones son del dueño** (CLAUDE.md, modo de un solo constructor). Nada de esto es consejo de inversión.

## Puntos para el dueño

| # | Parámetro (`config/risk.toml`) | Antes (apagado) | Recomendado | Por qué | Estado |
|---|---|---|---|---|---|
| 1 | `[[correlation.groups]]` `us_equity_index` = US100, US500, US30, US2000 | sin grupos | `max_risk_pct = "1.5"` | Los cuatro índices de EE. UU. se mueven casi igual: hoy 5 posiciones al 1 % en ellos son casi una sola apuesta del 5 %. Con 1,5 % entran una entera y media; la segunda se achica | **APROBADO** (dueño, 25-11-2026: «1, 2 y 7 apruebo la recomendacion.»; en `config/risk.toml` desde #689) |
| 2 | Otros grupos (DE40, XAUUSD) | — | ninguno por ahora | DE40 y el oro no tienen pareja en el catálogo; un grupo de uno es un error de config | **APROBADO** (dueño, 25-11-2026: «1, 2 y 7 apruebo la recomendacion.»; en `config/risk.toml` desde #689) |
| 3 | `[gap] gap_atr` | `"0"` | `"0.25"` (provisional) | Un gap por noticias salta el stop. Con stops de 1 a 3 ATR, 0,25 ATR achica el tamaño entre 8 y 20 %, y `risk` pasa a contar ese salto. Conviene fijarlo con el deslizamiento real más allá del stop que mida el paper | **APROBADO** (dueño, 25-11-2026: «1, 2 y 7 apruebo la recomendacion.»; en `config/risk.toml` desde #689) |
| 4 | `[drawdown_scaling]` | sin puntos | `mode = "linear"`, puntos (5 %, 1), (10 %, 0,5), (12,5 %, 0,25) | Hasta 5 % de caída es ruido normal y no se toca. De 5 a 10 % el riesgo baja en línea recta del 1 % al 0,5 %, y al 12,5 % llega a 0,25 %. El kill switch del 15 % no cambia. Así una mala racha frena antes de tocar el límite, en vez de todo o nada | **APROBADO** (dueño, 25-11-2026: «1, 2 y 7 apruebo la recomendacion.»; en `config/risk.toml` desde #689) |
| 5 | `[vol_target]` | objetivo `"0"` (apagada) | `target_annual_pct = "10"`, `min_scale = "0.5"`, `max_scale = "1"` | Con la cuenta más volátil que el objetivo, el riesgo baja hasta la mitad. `max_scale = 1` hace que **nunca suba** por encima del 1 % por operación. Requiere antes conectar la volatilidad realizada (ver abajo) | **PENDIENTE**: sigue apagada hasta que se calcule la volatilidad realizada de la cuenta (#692) |

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

### 3. Reducción gradual por caída (#671)

- Puntos (caída desde el máximo en %, factor). Se empieza implícitamente en (0 %, 1).
  - `linear`: interpola en línea recta entre puntos.
  - `step`: escalones; desde cada punto vale su factor.
  - Después del último punto vale el último factor, hasta el kill switch de `max_drawdown_pct`, que no cambia.
- Validación al cargar: caída creciente, factor que nunca sube (≤ 1) y todos los puntos antes de `max_drawdown_pct`.
- El factor multiplica `risk_per_trade_pct`. `Sizing.risk_scale` muestra el factor final.

### 4. Volatilidad objetivo de la cuenta (#671)

- Factor = `target_annual_pct` / volatilidad anual realizada del capital, acotado a [`min_scale`, `max_scale`]. Se multiplica con el de la caída.
- La volatilidad realizada la pasa quien llama (`Account.realized_vol_pct`). Sin dato (calentamiento), factor 1 acotado a los topes.
- **Hoy nadie la calcula:** el broker simulado y los backtests pasan `None`. Antes de prenderla hace falta un Issue aparte que la calcule del capital diario (por ejemplo, desvío de 60 días × √252) en el broker y en el backtest de cartera. Sin eso, prenderla no tendría efecto.
- Apagada = objetivo 0 con los topes en 1 (la config lo exige).

## Qué sigue después de la aprobación

1. ~~El dueño elige los valores~~: aprobados el 25-11-2026 (puntos 1 a 4).
2. ~~Un PR aparte cambia `config/risk.toml`~~: #689. Los tests de la mecánica usan una config neutral; el de la config del repositorio fija los valores aprobados.
3. La corrida del backtest de cartera con y sin los topes: `docs/analysis/2026-11-riesgo-con-sin-topes.md`, registrada en `docs/analysis/registro-de-resultados.csv`.
4. Pendiente: #691 (el broker simulado actualiza el máximo del capital solo al decidir; con la caída gradual, paper y backtest pueden separarse) y #692 (la volatilidad realizada de la cuenta, requisito del punto 5).
