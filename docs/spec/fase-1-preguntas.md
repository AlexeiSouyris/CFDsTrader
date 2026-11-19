# Fase 1 — Preguntas para cerrar la especificación

Respondan **los dos** debajo de cada pregunta (o en el chat con su agente, que lo pasa acá).
Cuando no queden preguntas abiertas, el agente redacta `docs/spec/fase-1-especificacion.md` con reglas en pseudocódigo para aprobar.
Objetivo: **dos personas que lean la spec harían exactamente las mismas operaciones.**

> Recomendación: especificar primero **UNA estrategia base** bien definida (la que ustedes ya usan a mano) y agregar capas (noticias, análogos, ML) como filtros o ajustes de probabilidad medibles, para poder comparar con y sin cada capa.

## A. Mercado y broker
1. ¿Qué instrumentos para empezar? (por ejemplo US Tech 100, S&P 500, DAX, acciones puntuales, forex, oro, cripto)
   - Respuesta:
2. ¿Qué broker y qué país o regulación? (los límites de apalancamiento dependen del regulador; por ejemplo, en la UE con ESMA los índices principales llegan a 20:1)
   - Respuesta:
3. ¿Tienen cuenta demo con acceso a la API? (en la captura usan IG)
   - Respuesta:
4. ¿De dónde salen los datos históricos y en qué resolución (tick, 1m, 15m, diario)? ¿Hay presupuesto para datos pagos?
   - Respuesta:
5. ¿Timeframe de entrada y timeframe(s) de contexto? (por ejemplo tendencia en 1h o diario, entrada en 15m)
   - Respuesta:
6. ¿Horario en que el sistema puede operar, y en qué zona horaria están ustedes? ¿Solo la sesión de contado de EE. UU. o también el CFD 24 h?
   - Respuesta:
7. ¿Se mantienen posiciones overnight o de fin de semana (con swap y riesgo de gap)?
   - Respuesta:

## B. Rol del sistema
8. En simulación, ¿el sistema abre y cierra operaciones solo, o propone y ustedes confirman?
   - Respuesta:
9. Cuéntenme paso a paso **una operación real** que hayan hecho: qué vieron, por qué entraron, dónde pusieron el stop y el take profit, y cómo salieron. Es la mejor base para la estrategia inicial.
   - Respuesta:

## C. Entrada
10. ¿Qué patrones de velas cuentan? (en la captura: martillo y envolvente alcista con filtro SMA50) ¿Con qué filtro de tendencia?
    - Respuesta:
11. Doble techo o suelo: ¿la entrada es al romper el *neckline*, en el retesteo o en el segundo toque? ¿Qué diferencia máxima entre los dos topes (en %)?
    - Respuesta:
12. Liquidez interna y externa: ¿usan el marco ICT/SMC (*sweeps*, *order blocks*, *fair value gaps*, BOS/CHoCH)? Definan qué es para ustedes "tomar liquidez de afuera" y "de adentro".
    - Respuesta:
13. ¿Las opciones (volatilidad implícita, put/call, flujo, como en la captura de NDAQ) son una señal o solo contexto?
    - Respuesta:
14. ¿Qué tiene que pasar para **no** entrar? (noticias de alto impacto cerca, spread alto, baja liquidez, viernes tarde...)
    - Respuesta:

## D. Salida
15. Stop loss: ¿por estructura (debajo del mínimo del patrón), por ATR o por un % fijo?
    - Respuesta:
16. Take profit: ¿múltiplo de riesgo (por ejemplo 2R), en la siguiente zona de liquidez, parcial y trailing?
    - Respuesta:
17. ¿Hay salida por tiempo (cerrar si en N velas no se movió) o antes de noticias o del cierre?
    - Respuesta:

## E. Tamaño, apalancamiento y riesgo
18. Capital simulado inicial y moneda de la cuenta.
    - Respuesta:
19. ¿Cuánto riesgo por operación (% del capital), pérdida máxima diaria, drawdown máximo y posiciones abiertas máximas?
    - Respuesta:
20. ¿Cómo se elige el apalancamiento? (fijo por instrumento, según volatilidad, según la confianza de la señal) ¿Cuál es el máximo por instrumento?
    - Respuesta:
21. ¿A qué nivel de margen hace stop-out el broker?
    - Respuesta:

## F. Noticias
22. ¿Las noticias son un **filtro** (no operar), un **ajuste de probabilidad** o una **señal propia**?
    - Respuesta:
23. ¿Qué fuentes? (agencias, calendario económico, Fed, resultados de empresas, X, Reddit) ¿Hay presupuesto para fuentes pagas en tiempo real?
    - Respuesta:
24. ¿La interpretación la hace un LLM (por ejemplo Claude: mejor razonamiento, costo por noticia), un modelo propio, o los dos? ¿Y los resúmenes en qué idioma?
    - Respuesta:

## G. Análogos históricos
25. ¿Qué es una "situación similar"? (forma del gráfico, valores de indicadores, contexto macro, tipo de noticia) ¿Cómo debería cambiar la decisión el resultado de los análogos?
    - Respuesta:
26. ¿Qué universo y cuántos años? (S&P 500, NASDAQ 100, índices globales; incluir empresas que dejaron de cotizar)
    - Respuesta:

## H. "Matemática cuántica"
27. ¿Se refieren a computación cuántica real (por ejemplo Qiskit para optimización) o a modelos probabilísticos avanzados (Monte Carlo, cadenas de Markov, bayesianos, ML)? Cambia mucho el plan.
    - Respuesta:

## I. Costos y casos borde
28. Spread típico, comisión y swap overnight de cada instrumento en su broker (o dónde se consultan).
    - Respuesta:
29. ¿Qué hacer ante: gap de apertura que salta el stop, feriado, halt del subyacente, datos que faltan o llegan tarde, ejecución parcial, desconexión con una posición abierta?
    - Respuesta:

## J. Proceso
30. ¿Cuánto tiempo mínimo de paper trading antes de pensar en dinero real? ¿Quién aprueba cada fase (los dos)?
    - Respuesta:
