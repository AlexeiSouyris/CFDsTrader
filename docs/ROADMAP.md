# Roadmap por fases

Regla: **no se empieza una fase sin que la anterior esté aprobada por las dos personas** (en el Issue de la fase). Mientras el dueño construya solo, alcanza con su aprobación (`CLAUDE.md`, modo de un solo constructor). Para la fase 11 (dinero real) siempre hacen falta dos personas.
Cada entrega incluye qué se hizo, código, tests, cómo verificarlo, supuestos y riesgos, y el checklist de trading de la plantilla de PR.

Primero va el motor y la inteligencia. La UI viene después (fase 10).

| # | Fase | Dónde | Estado |
|---|---|---|---|
| 0 | Fundaciones: reglas, arquitectura, ADRs | `docs/` | hecha (#1) |
| 1 | Especificación (sin código) | `docs/spec/` | aprobada (#1) |
| 2 | Datos | `engine/market-data`, `infra/` | aprobada (#2, 20-11-2026). La carga de historia de IG sigue (#102) |
| 3 | Motor de backtest | `engine/backtest` | aprobada (#74, 20-11-2026) |
| 3b | Análisis técnico: indicadores, patrones, liquidez, multi-timeframe | `engine/indicators`, `engine/patterns`, `engine/liquidity` | aprobada (#36, 21-11-2026) |
| 4 | Estrategia | `engine/strategy` | aprobada (#86, 21-11-2026) |
| 4b | Inteligencia: noticias, análogos históricos, ML | `intelligence/` | aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419); criterio pendiente: "mejora medida fuera de muestra frente a no usar la capa" (#99) |
| 5 | Evaluación y anti-sobreajuste | `engine/backtest` + `intelligence/` | aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419); criterio pendiente: "se sostiene fuera de muestra y con costos realistas". Adelantos: métricas (#58), financiación con el €STR diario (#79) |
| 6 | Gestión de riesgo (independiente) | `engine/risk` | aprobada (#350, 24-11-2026, fuera de orden) |
| 6b | Fondo del grupo: aportes, unidades, NAV, P&L por miembro | `engine/fund` | aprobada (#351, 24-11-2026, fuera de orden) |
| 7 | Capa de broker (simulado / real intercambiables) | `engine/market-data`, `engine/execution` | aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419); criterio pendiente: "funciona igual contra el simulador y contra la cuenta demo del broker" (#278). Adelanto: proveedor de IG de solo lectura (ADR 0002) |
| 8 | Paper trading en tiempo real | todo el motor | aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419); criterio pendiente: "la diferencia está explicada", sobre un período real de paper. Adelanto: velas en vivo por REST (`ig_live`, #68) |
| 8b | Aprendizaje continuo: diario, calibración, aporte por señal, reentrenamiento | `engine/journal`, `intelligence/learning` | aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419); criterio pendiente: "el informe se genera con los datos del paper trading, la calibración se puede verificar a mano con una muestra, y ningún parámetro cambia sin un voto registrado" (#327). Adelanto: diario de propuestas (#60) |
| 9 | Observabilidad y operación segura | `engine/`, `infra/` | aprobada (#352, 24-11-2026, fuera de orden). Telegram, a decisión del dueño |
| 10 | UI en TypeScript | `ui/` | aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419); criterio pendiente: sin criterio escrito, falta el alcance de cuentas con 2FA, push, votaciones y acceso desde el celular (#275). Adelanto: prototipo con datos de ejemplo (#30) |
| 11 | Dinero real (requiere ADR firmado por ambos) | — | bloqueada: la aprobación del dueño de #419 no alcanza, pide un ADR de dos personas |

"Adelanto" = trabajo hecho antes de que su fase empiece formalmente.

"Aprobada para avanzar" = el dueño autorizó trabajar en la fase (24-11-2026, #419: «te apruebo los 11 ya ahora para que hagas lo que necesites con todo mi permiso»), pero su criterio de aprobación **no** está cumplido. La fase se da por cumplida recién cuando se cumpla su criterio. Detalle en `docs/estado-de-fases.md`.

---

## Fase 1 — Especificación (sin código)
Mercado, instrumentos, timeframes y horario (con zona horaria). Reglas exactas de entrada, salida, stop loss y take profit en pseudocódigo. Tamaño de posición, apalancamiento por instrumento y límites de riesgo (por operación, por día, drawdown máximo). Casos borde: gaps, feriados, halts, datos faltantes, ejecuciones parciales, swap overnight, margin call o stop-out del broker.
**Aprobación:** dos personas que lean la spec harían exactamente las mismas operaciones.

## Fase 2 — Datos
Carga con validación: huecos, duplicados, precios ≤ 0, outliers y orden temporal. Ajuste por splits y dividendos, con la fuente documentada. Todo en UTC internamente. Separación entrenamiento / validación / prueba fuera de muestra (la prueba no se toca hasta el final). Almacenamiento en PostgreSQL y publicación en tiempo real por Redis.
**Aprobación:** tests que fallan con datos corruptos a propósito.

## Fase 3 — Motor de backtest
Sin look-ahead: decisión al cierre de `t`, ejecución en `t+1`. Comisión, spread, slippage y swap configurables y nunca en cero. Sin sesgo de supervivencia. Registro de cada operación: hora, precio, tamaño, apalancamiento, margen, motivo, costo y P/L.
**Aprobación:** comprar y mantener da un resultado calculable a mano.

## Fase 3b — Análisis técnico
- **Indicadores** (SMA/EMA, RSI, MACD, Bollinger, ATR, VWAP, volumen...): un módulo por indicador con la fórmula exacta documentada (por ejemplo, RSI con suavizado Wilder), validados contra TradingView en el mismo período, con calentamiento y sin repintado.
- **Patrones** (soportes y resistencias, tendencias, doble techo o suelo, hombro-cabeza-hombro, triángulos, rupturas, velas japonesas): definiciones objetivas y parametrizadas, más un test de sensibilidad de la tolerancia. Pivotes confirmados tras N velas.
- **Liquidez**: definición objetiva de zonas de liquidez interna y externa y de los barridos (*sweeps*). Fuente de verdad: la spec de fase 1.
- **Multi-timeframe**: solo velas cerradas del timeframe mayor, alineadas en UTC, con tests de resampleo.
- **Verificación visual**: gráficos con velas, indicadores y marcas de entrada, salida y stop, más una tabla por operación con los valores de cada indicador en la señal. Las herramientas de gráfico solo leen lo que exporta el motor, no recalculan.
**Aprobación:** coincide con la referencia, ningún valor histórico cambia al agregar datos, y una muestra de operaciones revisada en el gráfico cumple las reglas.

## Fase 4 — Estrategia
Señales separadas del backtest y de la ejecución. Funciones puras. Un test por cada regla de entrada y salida, con casos construidos a mano.
**Aprobación:** cada regla de la spec tiene al menos un test.

## Fase 4b — Inteligencia (Python)
- **Noticias**: ingesta con hora de publicación real; resumen, interpretación (positiva o negativa, magnitud, instrumentos afectados) y probabilidad de impacto, emitidos como evento por Redis.
- **Análogos históricos**: búsqueda de situaciones similares en otras empresas e índices, y la distribución de lo que pasó después (no un único "pronóstico").
- **ML**: modelos PyTorch entrenados solo con datos de entrenamiento y validación, y evaluados con walk-forward.
**Aprobación:** sin fuga de información (*leakage*) entre períodos, y mejora medida fuera de muestra frente a no usar la capa.

## Fase 5 — Evaluación y anti-sobreajuste
Retorno, drawdown máximo, Sharpe y Sortino, win rate, profit factor, número de operaciones y exposición. Walk-forward. Sensibilidad de parámetros. Benchmark de comprar y mantener el índice. Resultado fuera de muestra reportado aparte y sin retocar.
**Aprobación:** se sostiene fuera de muestra y con costos realistas.

## Fase 6 — Gestión de riesgo
Límites duros que la estrategia no puede saltear: tamaño máximo, apalancamiento máximo por instrumento, pérdida diaria máxima, drawdown máximo y posiciones abiertas máximas. Kill switch. Validación de cada orden (precio razonable, tamaño dentro de límites, no duplicada).
**Aprobación:** un test por límite que verifica que se dispara.

## Fase 6b — Fondo del grupo
Registro de aportes y retiros por miembro. Cada movimiento compra o vende unidades al valor de la unidad (NAV) del momento. P&L por miembro = unidades × NAV − aportes netos. Historial auditable y decimales exactos.
**Aprobación:** el ejemplo del glosario (Ana, Beto y Caro) da exactamente el resultado calculado a mano, y la suma de lo que vale cada miembro es igual al valor total del fondo.

## Fase 7 — Broker
Interfaz con implementación simulada y real intercambiables. Órdenes idempotentes. Manejo de timeouts, rechazos, reconexión y ejecuciones parciales. Reconciliación de posiciones al arrancar y periódicamente. Credenciales solo por variables de entorno, nunca en código ni en logs.
**Aprobación:** funciona igual contra el simulador y contra la cuenta demo del broker.

## Fase 8 — Paper trading
En tiempo real con dinero simulado durante un período mínimo (lo define la spec). Comparación contra el backtest del mismo período.
**Aprobación:** la diferencia está explicada.

## Fase 8b — Aprendizaje continuo
Diario de cada propuesta con su contexto y su resultado (simulado y real). Informe periódico con: calibración de las probabilidades mostradas, aporte de cada señal al resultado, y modelos reentrenados con walk-forward. **El sistema no se cambia solo**: el informe trae propuestas de cambio y cada una pasa por votación (spec §1.2). Detalle en spec §15b.
**Aprobación:** el informe se genera con los datos del paper trading, la calibración se puede verificar a mano con una muestra, y ningún parámetro cambia sin un voto registrado.

## Fase 9 — Observabilidad y operación segura
Logs estructurados de cada decisión y orden. Alertas (Telegram o email) ante errores, kill switch o desconexión. Arranque y parada seguros, sin órdenes huérfanas.

## Fase 10 — UI y app del celular (TypeScript)
App privada para los iPhone de los 6 miembros: notificaciones push, aprobación de propuestas, votaciones, cuentas individuales con 2FA y su P&L.
Gráfico en tiempo real con patrones y señales marcados, panel de noticias actualizado al segundo con su interpretación, estrategias con probabilidades, P/L, y edición de parámetros (que a su vez pasan por la validación de configuración y por el riesgo). Vista del fondo con el aporte, las unidades y el P&L de cada miembro. **Diccionario interactivo:** cada término técnico se puede tocar para ver su explicación y un ejemplo, tomados de `docs/GLOSARIO.md`.

## Fase 11 — Dinero real
Solo con ADR aprobado por ambos, después de un paper trading aprobado, y empezando con capital mínimo.
