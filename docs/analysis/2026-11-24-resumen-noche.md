# Resumen de la noche del 24-11-2026 (00:00 → 07:00 UTC)

Trabajo autónomo del agente coordinador con ~25 agentes en paralelo. Se mergearon **63 PRs** (#354 → #474), todos con el CI en verde y la plantilla completa. Nada toca dinero real ni credenciales.

## Lo más importante

1. **La estrategia actual no tiene ventaja, en ningún índice.** Sin costos queda en torno a 0 R por operación, y los costos del CFD (0,10–0,18 R) la vuelven perdedora.
   - EE. UU. con 15 años: #461 (diagnóstico), #411 (walk-forward) y #457 (US100 y Russell).
   - DAX: #324 y #373.
   - Ningún filtro (hora, día, disparador, régimen) da vuelta el signo en los tres índices.
2. **El ML confirma lo mismo.** Las variables actuales no predicen si una propuesta toca TP1 (AUC 0,525). Las variables nuevas probadas (tasas, VIX, análogos) tampoco mejoran. Ver #430 y #435.
3. **La ronda 1 de estrategias nuevas tampoco pasa** (#474). La ruptura del rango de apertura pierde y el cambio de mes da +0,03 %/mes, no significativo. Según el pre-registro, **toca revisar el planteo con vos** antes de la ronda 2.
4. **El paper trading quedó listo para el 27-11.** El ensayo (#443) encontró 7 errores y los 7 están arreglados (#444, #446, #458, #459, #460, #465, #469, #473). Falta solo tu decisión de capital (abajo).

## Datos (todo cargado y procesado)

| Mercado | Historia | Fuente |
|---|---|---|
| S&P, Nasdaq, Dow | 2011 → hoy | Databento (ES, NQ, YM) |
| Russell 2000 | 2017-07 → hoy | Databento (RTY) |
| Oro | 2011 → hoy | Databento (GC) |
| DAX | 2025-03 → hoy | Databento (FDAX) |
| EUR/USD | 2011 → hoy | BCE |
| EFFR / SOFR | 2010 → hoy | NY Fed |

- Velas de 15m/1h, diarias e indicadores armados para todos.
- Créditos de Databento usados esta noche: ~USD 71 de 94,63.

## UI

Gráficos navegables (arrastrar, rueda, pellizco), modo línea con área degradada, punto que sigue la vela, medición con pestaña, gestos del celular, buscador arreglado, transición de entrada con portales, QA de todas las vistas y tarjeta «Modelo (informativo)» con calibración.

## Oro (ADR 0013, aprobado)

Ya está implementado:
- clase de activo «materias primas»;
- sesión de 23 h;
- fórmula de financiación de IG;
- conversión con el EUR/USD vigente;
- tope 10:1;
- XAUUSD en el catálogo.

**Todavía no opera.** Faltan tus horas de entrada y datos que IG no publica: epic, tamaño mínimo, base de financiación y comisión de cambio.

## Decisiones que necesito de vos

1. **Capital del paper trading.** Con 1.000 €, el DAX rechaza el 100 % de las propuestas por tamaño mínimo o stop. Recomiendo **10.000 €** simulados (no es plata real). Sin esto, el runner del 27-11 no va a abrir nada.
2. **Cómo seguir con la estrategia.** Ver #466 y #474. Opciones:
   - (a) ronda 2: tendencia de baja frecuencia, que pide más capital para el tamaño mínimo de IG;
   - (b) replantear qué se predice;
   - (c) aceptar que con CFD de IG y esta frecuencia los costos se comen cualquier ventaja chica.
3. **Horas de entrada del oro** (#418).
4. **ML v2:** cerrarlo por ahora (mi recomendación) o abrir otra familia de variables.
5. **ADR 0014 de la fase 10** (cuentas con TOTP, push, votos y Tailscale): 5 decisiones al final del ADR. Leer los términos de Tailscale (uso no comercial).

## Para hacer vos (no lo puede hacer un agente)

- **Mandar los mails de `docs/guides/consultas-proveedores.md`.**
  - A IG API support: habilitar el *streaming*.
  - A IG: tasa de financiación de los índices de EE. UU. y confirmar que **IG España ajusta dividendos** en los CFD de índices (#470).
- **Crontab:** agregar la carga diaria del EUR/USD (línea en la guía de uso, sección 6). Las tasas de la NY Fed entran solas con la tarea `rates` que ya tenés.
- **IBKR:** esperar la respuesta del soporte antes de depositar.
- **Liberar el kill switch de paper** (si alguna vez salta): ahora persiste y se libera solo a mano, con el gateway parado:
  ```sql
  INSERT INTO paper_kill_switch (change, after_orders, reason, at)
  VALUES ('released', (SELECT count(*) FROM paper_orders), '<motivo / votación>', now());
  ```

## El 27-11 (cuando se renueva el cupo de IG)

1. Verificar que `ig_live` vuelve a guardar velas y spreads.
2. Mergear #382 (runner prendido), con el capital decidido, y reiniciar el gateway.
3. El runner necesita unas 3 sesiones de velas antes de su primera decisión (calentamiento y spreads).
4. Desde ahí cuentan los 3 meses de paper de la fase 8.

## Incidentes de la noche

- **Filas de prueba en la base real.** Un agente corrió tests contra la base real y dejó 20 filas sintéticas.
  - Se respaldaron en `~/cfds-backups/` y se borraron con tu OK. El primer borrado no se confirmó (transacción sin commit); el segundo sí, verificado.
  - No afectaron ningún cálculo.
  - Para que no vuelva a pasar: los tests ahora se niegan a correr si la base no es `cfds_test*` (#381).
- **Tercios de volatilidad vacíos.** Se completaron una sola vez (NULL → valor) con auditoría en `daily_null_fills` (#427), aprobado como recomendación.
