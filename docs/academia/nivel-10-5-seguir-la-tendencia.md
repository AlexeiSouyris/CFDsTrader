# 10.5 Seguir la tendencia

> **Nivel 10 · Tu propia estrategia.** Educativo: no es consejo de inversión ni promete ganancias.
> Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son resultados del
> pasado, propios y publicados: no dicen nada seguro del futuro.

## Objetivo

Conocer la evidencia **más larga** del trading sistemático, seguir la
[tendencia](../GLOSARIO.md#tendencia) en muchos mercados, y entender por qué nuestra prueba en tres
índices **no pasó**.

## La idea

**1. Lo que dice la evidencia publicada.**

- **Moskowitz, Ooi y Pedersen (2012)**, «momentum de serie de tiempo»: en futuros de índices de
  bolsa, monedas, materias primas y bonos, el retorno de los últimos 1 a 12 meses de cada mercado
  tiende a **seguir**, y después se revierte en parte. Lo encontraron en cada uno de los 58
  instrumentos líquidos que estudiaron.
- **Hurst, Ooi y Pedersen (2017)** llevan la misma idea hasta **1880** y la encuentran rentable de
  forma consistente en más de un siglo.

Es la evidencia más sólida que hay para una regla simple. Ojo con lo que **no** dice: que funcione
en un solo mercado, en velas cortas o todos los años.

**2. Nuestra prueba (R016).** Tendencia de 12 meses en US500, US100 y US30, de 2013 a 2025: largo si
el retorno de 12 meses es positivo y corto si es negativo, revisado una vez por mes, con los costos
del CFD (y cerrando cada viernes para reabrir el lunes, como fijó su regla).

- Resultado: **+0,080 %** por mes, con **p 0,14** (hacía falta menos de 0,0125). Positivo, pero no
  se distingue del azar.
- Su Sharpe fue **un tercio** del de comprar y mantener en los mismos meses (0,087 contra 0,261
  mensual). Las dos mitades fueron positivas y aguantó los costos × 1,5, pero no alcanza.

**3. Por qué no contradice a la literatura.** La evidencia usa **muchos** mercados distintos.
Nuestros tres índices de EE. UU. se mueven casi igual: son casi una sola apuesta. Una regla de
tendencia gana poco en cada mercado y muchas tendencias más o menos independientes son las que
suman. Además, en una década alcista, quedarse siempre comprado es difícil de superar.

**4. Lo que viene: el programa 6 (borrador).** La propuesta del autoanálisis es pasar a horizontes
diarios y a **16 mercados** (bolsas, bonos, monedas y materias primas), donde el costo pesa menos de
0,02 R por operación y los datos ya están descargados:

- regla estilo Carver: tres cruces de medias exponenciales más una ruptura, con parámetros
  publicados y sin ajustar nada, y riesgo repartido por volatilidad;
- el criterio **no** es ganarle al S&P 500 en retorno: es tener ventaja propia (alfa) frente a él
  y no hundirse cuando la bolsa se hunde;
- está en **borrador, sin congelar**: no tiene `.sha256` ni parámetros en la config, y no se corre
  nada hasta que el dueño lo apruebe;
- su apuesta sale del presupuesto global: α ≤ 1,25 %. Con un α tan chico, una cartera de tendencia
  buena pero normal **puede no pasar**: el método protege contra falsos positivos a costa de
  perder alguno verdadero.

> **Probá antes de seguir:** si la tendencia es la mejor evidencia publicada, ¿por qué no se opera
> directamente, sin probarla?
>
> <details><summary>Ver la respuesta</summary>
>
> Porque lo publicado es en otros mercados, otros períodos y otros costos. Las anomalías publicadas
> pierden buena parte de su efecto después de publicadas (lección 10.2), nuestro vehículo es un CFD
> con financiación nocturna, y R016 ya mostró que en tres índices no alcanza. Se prueba con su
> pre-registro, como todo.
>
> </details>

## Interactivo

La tendencia de 12 meses de los tres índices sobre el gráfico diario, con R016 al lado (antes y
después de costos, y contra comprar y mantener).

## Ejercicio

Sin nota: ¿por qué una regla de tendencia que acierta pocas veces puede igual tener expectativa
positiva? Usá lo del nivel 8.

<details><summary>Ver la solución</summary>

Porque deja correr las ganadoras (tendencias largas) y corta las perdedoras: con pocas ganadoras
grandes, p × G puede ser mayor que (1 − p) × P. El precio: rachas de pérdidas largas (8.2), que
hay que aguantar con tamaño chico.

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada nuevo.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Persistencia de 1 a 12 meses, reversión parcial, 58 instrumentos | — | [Moskowitz, Ooi y Pedersen (2012), *Time Series Momentum*, Journal of Financial Economics 104(2), 228–250](https://ideas.repec.org/a/eee/jfinec/v104y2012i2p228-250.html), resumen leído el 26-11-2026 | A |
| Tendencia desde 1880, rentable en más de un siglo | — | [Hurst, Ooi y Pedersen (2017), *A Century of Evidence on Trend-Following Investing*, Journal of Portfolio Management](https://www.aqr.com/Insights/Research/Journal-Article/A-Century-of-Evidence-on-Trend-Following-Investing), resumen leído el 26-11-2026 | A |
| R016: retorno, p y Sharpe frente a comprar y mantener | `+0.080 % (p 0.14; 0.087 vs 0.261)` | `docs/analysis/registro-de-resultados.csv` `R016` | A |
| Nivel de cada hipótesis | 1,25 % (`0.0125`) | `config/research.toml` `rule_round2.alpha` | A |
| Sharpe: un tercio del de comprar y mantener | `su Sharpe (≈ 0,30 anual) es` | `docs/analysis/2026-11-ronda2-tendencia-y-momentum-intradia.md` | A |
| La regla cierra cada viernes | `Cierra cada viernes y reabre el lunes` | `docs/analysis/2026-11-ronda2-tendencia-y-momentum-intradia.md` | A |
| Los tres índices se mueven casi igual | `que se mueven casi igual` | `docs/analysis/2026-11-autoanalisis.md` | A |
| En diario el costo pesa poco; 16 mercados descargados | `Ahí el costo pesa menos de 0,02 R por operación` | `docs/analysis/2026-11-autoanalisis.md` | A |
| 16 futuros diarios desde 2011 | `16 futuros diarios desde 2011` | `docs/analysis/2026-11-autoanalisis.md` | A |
| Programa 6: borrador | `Estado: BORRADOR, sin congelar` | `docs/analysis/2026-11-programa-6-tendencia-preregistro.md` | A |
| Programa 6: sin parámetros en la config | `ni parámetros del programa en la configuración` | `docs/analysis/2026-11-programa-6-tendencia-preregistro.md` | A |
| Programa 6: el criterio no es el retorno | `No se pide "ganarle al S&P 500 en retorno"` | `docs/analysis/2026-11-programa-6-tendencia-preregistro.md` | A |
| Programa 6: apuesta del presupuesto global | `α₆ ≤ 1,25 %` | `docs/analysis/2026-11-programa-6-tendencia-preregistro.md` | A |
| Con α chico puede no pasar | `una cartera de tendencia buena pero normal puede no pasar` | `docs/analysis/2026-11-programa-6-tendencia-preregistro.md` | A |
