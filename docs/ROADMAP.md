# Roadmap por fases

Regla: **no se empieza una fase sin que la anterior esté aprobada por las dos personas** (en el Issue de la fase).
Cada entrega incluye qué se hizo, código, tests, cómo verificarlo, supuestos y riesgos, y el checklist de trading de la plantilla de PR.

Primero va el motor y la inteligencia. La UI viene después (fase 10).

| # | Fase | Dónde | Estado |
|---|---|---|---|
| 0 | Fundaciones: reglas, arquitectura, ADRs | `docs/` | en curso |
| 1 | Especificación (sin código) | `docs/spec/` | en curso: preguntas abiertas |
| 2 | Datos | `engine/market-data`, `infra/` | pendiente |
| 3 | Motor de backtest | `engine/backtest` | pendiente |
| 3b | Análisis técnico: indicadores, patrones, liquidez, multi-timeframe | `engine/indicators`, `engine/patterns`, `engine/liquidity` | pendiente |
| 4 | Estrategia | `engine/strategy` | pendiente |
| 4b | Inteligencia: noticias, análogos históricos, ML | `intelligence/` | pendiente |
| 5 | Evaluación y anti-sobreajuste | `engine/backtest` + `intelligence/` | pendiente |
| 6 | Gestión de riesgo (independiente) | `engine/risk` | pendiente |
| 7 | Capa de broker (simulado / real intercambiables) | `engine/market-data`, `engine/execution` | pendiente |
| 8 | Paper trading en tiempo real | todo el motor | pendiente |
| 9 | Observabilidad y operación segura | `engine/`, `infra/` | pendiente |
| 10 | UI en TypeScript | `ui/` | pendiente |
| 11 | Dinero real (requiere ADR firmado por ambos) | — | bloqueada |

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

## Fase 7 — Broker
Interfaz con implementación simulada y real intercambiables. Órdenes idempotentes. Manejo de timeouts, rechazos, reconexión y ejecuciones parciales. Reconciliación de posiciones al arrancar y periódicamente. Credenciales solo por variables de entorno, nunca en código ni en logs.
**Aprobación:** funciona igual contra el simulador y contra la cuenta demo del broker.

## Fase 8 — Paper trading
En tiempo real con dinero simulado durante un período mínimo (lo define la spec). Comparación contra el backtest del mismo período.
**Aprobación:** la diferencia está explicada.

## Fase 9 — Observabilidad y operación segura
Logs estructurados de cada decisión y orden. Alertas (Telegram o email) ante errores, kill switch o desconexión. Arranque y parada seguros, sin órdenes huérfanas.

## Fase 10 — UI (TypeScript)
Gráfico en tiempo real con patrones y señales marcados, panel de noticias actualizado al segundo con su interpretación, estrategias con probabilidades, P/L, y edición de parámetros (que a su vez pasan por la validación de configuración y por el riesgo).

## Fase 11 — Dinero real
Solo con ADR aprobado por ambos, después de un paper trading aprobado, y empezando con capital mínimo.
