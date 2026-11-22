# 0003 – Modelo adaptativo por régimen, capas de contexto y blindaje anti-sobreajuste

- **Estado:** aceptada
- **Fecha:** 2026-11-20
- **Aprobada por:** las dos personas, con su aprobación registrada en el PR #133 (que no se mergea sin ella). Los puntos que cambian reglas de riesgo o parámetros también pasan por la votación de 4 de 6 (spec §1.2).
- **Actualizada por:** el [ADR 0004](0004-fuentes-de-sentimiento.md), que deja el VIX, el put/call y la AAII fuera de v1, y el #25 (spec §9.1), que pone la capa de miedo y sentimiento en el módulo `sentiment` y no en `news`.
- **Issue:** #19
- **Base:** [Por qué fallan los algoritmos de trading](../research/por-que-fallan-los-algoritmos.md)

## Contexto

La mayoría de los algoritmos minoristas fallan por sobreajuste, cambios de régimen, riesgo concentrado y ventajas que se gastan. Ver la tabla de patrones de error del documento de investigación. La spec de fase 1 cubre bien el look-ahead, los costos, el apalancamiento y los datos, pero tiene huecos:
- no se adapta al régimen de mercado;
- no corrige por la cantidad de variantes probadas;
- no limita el riesgo entre instrumentos correlacionados;
- no tiene un circuito claro para sumar contexto nuevo (miedo del mercado, política, calendario, IPOs) sin sobreajustar.

El grupo además pidió que el modelo sea **cambiante**. La spec §15b dice que el sistema **nunca cambia solo sus parámetros**. Este ADR propone una forma de compatibilizar las dos cosas.

## Decisión (propuesta)

### A. Detector de régimen de mercado (Rust, módulo nuevo `regime`)
- Clasifica cada instrumento, al cierre de cada vela de 1h, en un régimen con dos ejes objetivos:
  - **Volatilidad:** percentil del ATR(14) de 1h sobre los últimos 250 días, y la banda de VIX ya definida en la spec §13 (<15, 15–25, >25). Bandas: baja, normal, alta y extrema.
  - **Tendencia:** la tendencia de 1h de la spec (alcista, bajista o neutral).
- Solo velas cerradas, sin repintado, con calentamiento, y todos los umbrales en `config/`.
- El régimen se guarda en el diario con cada propuesta.

### B. Juegos de parámetros por régimen: "cambiante", pero con reglas votadas
- En vez de un solo juego de parámetros, puede haber **un juego por régimen**: umbral del puntaje, multiplicador del stop, riesgo por operación (siempre ≤ al límite del riesgo) e instrumentos habilitados.
- El sistema **elige automáticamente** el juego que corresponde al régimen actual. Esa elección es una función pura del régimen: misma entrada, misma salida.
- Cada juego **se valida walk-forward por separado y se vota** (4 de 6). El sistema cambia de comportamiento según el mercado, pero **nunca inventa ni ajusta parámetros por su cuenta**. Así se respeta §15b.
- Existe siempre el régimen **"no operar"**. Valor inicial propuesto: volatilidad extrema.
- Un juego solo se habilita con **≥ 30 operaciones fuera de muestra** en ese régimen. Hasta entonces se usa el juego base. Esto evita tener demasiados juegos con pocos datos.

### C. Capas de contexto (experimentales hasta probar valor)
Cada capa sigue el mismo circuito que la noticia como señal propia (spec §9): **se registra y se mide, pero no suma al puntaje** hasta demostrar valor fuera de muestra en la fase 5 y ganar una votación.

| Capa | Dónde | Qué emite |
|---|---|---|
| **Miedo / sentimiento del mercado** | Python (`news`) consume los datos; Rust solo los usa como contexto | VIX y estructura, put/call y encuestas de sentimiento, cada uno con su hora de publicación |
| **Eventos políticos y sociales** (protestas, elecciones, sanciones, aranceles) | Python (`news`, LLM) | Evento con tipo, países, sectores e instrumentos afectados, dirección, intensidad e incertidumbre |
| **Calendario** (feriados, Black Friday y Cyber Monday, vencimiento de opciones, cierre de trimestre, FOMC y BCE) | Rust, sobre el calendario de #9, con las fechas en `config/` | Etiquetas de calendario por vela |
| **Eventos corporativos** (IPOs, fin de lock-up, rebalanceos de índices, resultados) | Rust (`config/` y datos de mercado); el texto lo interpreta Python | Evento con fecha conocida de antemano |

Reglas de la capa:
1. **Hipótesis escrita antes de mirar los datos:** qué se espera, en qué instrumento, en qué ventana y con qué signo. Ejemplo: "el fin del lock-up de una IPO de IA del NASDAQ 100 se asocia con un retorno de 3 días por debajo del sector".
2. Solo se usa la información disponible en su **hora de publicación**.
3. Si demuestra valor, entra al puntaje con un peso chico (±1), sin superar el ajuste máximo de noticias de la spec (±2), salvo que se vote otra cosa.
4. Las fuentes nuevas se agregan con un ADR de acceso legal, como exige §9. **No se inventan endpoints.**

### D. Blindaje anti-sobreajuste (fase 5)
1. **Registro de experimentos:** cada variante probada (parámetros, señales, régimen) queda registrada con fecha y resultado, **incluidas las que fallaron**. Sin registro no hay validación.
2. **Sharpe deflactado** (Bailey y López de Prado, 2014): corrige el Sharpe por la cantidad de variantes probadas del registro.
3. **PBO** (probabilidad de sobreajuste del backtest, con validación cruzada combinatoria). Valor inicial propuesto: se rechaza si PBO > 0,5.
4. **Línea base aleatoria:** la estrategia tiene que superar a entradas aleatorias con los mismos stops, salidas, costos y frecuencia (Monte Carlo, ≥ 1.000 corridas), además de comprar y mantener.
5. **Meseta de parámetros:** un parámetro es aceptable solo si los valores vecinos también funcionan, no si es un pico aislado.

### E. Límite de riesgo por grupo correlacionado (módulo `risk`)
- Grupos en `config/`, valores iniciales: *Índices EE. UU.* (US Tech 100, US 500, Wall Street), *Índices Europa* (DAX, Euro Stoxx 50) y *Acciones de IA del NASDAQ 100*.
- **Riesgo abierto máximo por grupo: 2 %**, y en total el que ya fija la spec.
- Además, se revisa mensualmente la correlación real de 60 días, y el informe propone cambios a los grupos para votar.

### F. Monitor de deriva con pausa automática
- Con las operaciones del backtest walk-forward se arma una banda esperada (Monte Carlo) del resultado acumulado de las últimas N operaciones (N inicial: 20).
- Si el resultado en paper queda **por debajo del percentil 5** de esa banda, el sistema **deja de proponer** operaciones nuevas en ese juego de parámetros y alerta. Las posiciones abiertas siguen con su stop.
- Reactivar requiere una votación, como el kill switch (§1.2). Es una acción de **seguridad**: el sistema se frena, no cambia parámetros.

### G. Lo que NO se hace
- **No se agrega azar a las decisiones** para que el sistema sea "impredecible". No aporta ventaja, hace imposible medir el sistema y rompe la regla de funciones puras. Con el tamaño del fondo nadie puede estudiar nuestras órdenes. La protección real es que la estrategia sea privada.
- **No se usan creencias de calendario como reglas.** Por ejemplo, "en Navidad baja el mercado": la evidencia es mixta o contraria (ver la investigación §3.1). Se miden como contexto.
- **El sistema no se reentrena ni se reajusta solo** (se mantiene §15b).

## Alternativas consideradas

1. **Modelo que se reentrena solo en línea** (*online learning*). Es más "cambiante", pero se ajusta al ruido reciente, no se puede auditar y saltea la votación. Descartada.
2. **Un solo juego de parámetros** (spec actual). Es más simple, pero esconde el cambio de régimen, que es una de las causas de fracaso más comunes.
3. **Sumar todas las capas de contexto directo al puntaje.** Es más rápido, pero es el camino clásico al sobreajuste (error n.º 10). Descartada.

## Consecuencias

- **Más fácil:** entender **cuándo** funciona la estrategia (resultado por régimen), frenarse sola cuando deja de funcionar, y sumar ideas nuevas sin romper lo que anda.
- **Más difícil:** se necesitan más datos. Cada régimen necesita sus propios ≥ 30 casos, y algunos regímenes, como la volatilidad extrema, van a tener pocos datos por años.
- **Cambios en la spec** que habría que hacer si se aprueba, cada uno en su Issue:
  - §5 y §6: régimen y juegos de parámetros;
  - §9 y §13: capas de contexto experimentales;
  - §10: límite por grupo correlacionado y monitor de deriva;
  - §15: Sharpe deflactado, PBO y línea base aleatoria;
  - glosario: régimen, Sharpe deflactado y PBO.
- **Contratos nuevos** (contratos primero, en el PR de cada módulo): `regime.v1` y `context_event.v1`.
- **Módulos nuevos planeados:** `regime` (Rust). Las capas de contexto viven en los módulos que ya existen (`news` en Python, calendario en `market-data`).
- **Orden:** nada de esto adelanta fases. Calendario y régimen entran en la fase 3b, las capas de contexto en la 4b, el blindaje en la 5, el límite de correlación en la 6 y el monitor de deriva en la 8 o la 8b.
