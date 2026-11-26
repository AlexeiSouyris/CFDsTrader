# 11.3 GEX (exposición gamma): qué es y por qué no la medimos

> **Nivel 11 · Avanzado, con advertencias.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los números
> de los ejemplos son redondos, de práctica: no son cotizaciones ni resultados.

## Objetivo

Entender qué intenta medir la **exposición gamma** (GEX), por qué cualquier número de GEX es una
**estimación con supuestos**, qué dice la evidencia (el régimen sí, las «paredes» casi nada) y por
qué hoy no la podemos medir.

## La idea

**1. Delta, gamma y la cobertura.** El *delta* de una opción dice cuánto cambia su precio si el
activo sube 1 punto; la *gamma* dice cuánto cambia el delta. Un creador de mercado (*dealer*) que
vende o compra una opción suele cubrirse con el activo para quedar neutro en delta, y cada vez que
el precio se mueve tiene que **rehacer** esa cobertura:

| El dealer está... | Si el precio sube, se cubre... | Si baja... | Efecto posible |
|---|---|---|---|
| **Corto en gamma** (vendió opciones) | comprando | vendiendo | **empuja** el movimiento |
| **Largo en gamma** (compró opciones) | vendiendo | comprando | **frena** el movimiento |

**2. GEX es una estimación.** Para sumar la gamma de todos los dealers hace falta saber **quién
tiene cada opción**, y el dato público (el interés abierto por strike) no lo dice: cada contrato
abierto tiene un comprador y un vendedor. Por eso los números de GEX que circulan se calculan con
**supuestos**. El documento que popularizó la sigla (SqueezeMetrics, 2016, revisado en 2017) usa
cuatro, en resumen:

1. todas las opciones pasan por alguien que se cubre en delta;
2. las *calls* las venden los inversores y las compran los dealers;
3. las *puts* las compran los inversores y las venden los dealers;
4. los dealers se cubren exactamente al delta.

Con eso, por strike: GEX ≈ gamma × interés abierto × 100 (el multiplicador del contrato), con el
signo que dicen los supuestos. Si un día los inversores compran *calls* en masa, el supuesto 2 se
da vuelta y el **signo** de la estimación puede estar mal. Es un documento de un proveedor que
vende el dato: prueba **C**.

**3. Lo que dice la evidencia.**

- **El régimen tiene evidencia seria.** Ni, Pearson, Poteshman y White (2021, *Review of Financial
  Studies*) muestran que el rebalanceo de cobertura de los creadores de mercado afecta la
  volatilidad de las acciones y la probabilidad de movimientos grandes, aun sin información nueva.
  Baltussen, Da, Lammers y Martens (2021, *Journal of Financial Economics*), con más de 60 futuros
  entre 1974 y 2020: la última media hora tiende a seguir al resto del día, y lo ligan a la
  cobertura de gamma de creadores de mercado de opciones y de ETF apalancados. Barbon y Buraschi
  (documento de trabajo, revisado en 2021) encuentran impulso intradía con gamma negativa y reversión
  con gamma positiva, más fuerte en lo menos líquido; ellos también usan una **aproximación** de la
  gamma.
- **El vencimiento:** el día que vencen las opciones, el precio de las acciones con opciones tiende
  a terminar cerca de un strike (Ni, Pearson y Poteshman, 2005).
- **Las «paredes» casi no tienen evidencia.** Que el strike con más gamma frene el precio como un
  soporte o una resistencia, fuera del vencimiento, no está en esos estudios; en nuestra revisión,
  la única hipótesis de gamma con buena evidencia académica es la de Baltussen. Además, en el S&P
  500 los strikes van cada 5 puntos: «cerca de un strike» se cumple casi siempre.

**4. Lo que dio con nuestros datos.** La hipótesis de Baltussen (la última media hora sigue al día)
ya la probamos en US500, US100 y US30: **antes de costos** quedó en cero o negativa (−0,010 /
−0,013 / −0,031 R por operación, R026) y **con los costos del CFD** perdió −2,0 % por mes (R017).
Un efecto real en los papers puede no alcanzar para pagar el spread.

**5. Por qué hoy no la medimos.**

- No tenemos **ningún dato de opciones** (ni interés abierto por strike, ni gamma, ni quién compra).
- El dato que separa clientes, profesionales y creadores de mercado (*Open-Close* de Cboe) es un
  producto **pago**, con licencia de uso interno: el dato crudo no se puede redistribuir.
- Los términos del sitio de Cboe no permiten guardar sus datos sin permiso escrito (ADR 0012).
- Sumar un proveedor de datos va con su ADR (CLAUDE.md §6). Hasta entonces, nada de GEX en la app.

> **Probá antes de seguir:** una página muestra «GEX: −2.000 millones» para el S&P 500. ¿Qué te
> dice y qué no?
>
> <details><summary>Ver la respuesta</summary>
>
> Dice que, **según sus supuestos** sobre quién tiene cada opción, los dealers estarían cortos en
> gamma: un régimen en el que la cobertura podría empujar los movimientos. No dice dónde va a ir el
> precio, ni que un strike vaya a frenarlo, y si los supuestos fallan hasta el signo puede estar mal.
>
> </details>

## Interactivo

Solo el diagrama de arriba (corto en gamma empuja, largo en gamma frena): sin datos de opciones no
hay nada que dibujar con números reales.

## Ejercicio

Un dealer vendió *puts* y se cubre en delta. El precio baja fuerte. ¿Compra o vende el activo para
cubrirse, y qué le hace eso al movimiento?

<details><summary>Ver la solución</summary>

Vende. Al bajar el precio, el delta de la *put* vendida crece y el dealer tiene que vender más del
activo para seguir neutro: está corto en gamma y su cobertura **empuja** la baja.

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada: no hay datos de opciones. Con esta lección termina el nivel 11.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Cuatro supuestos de la GEX y GEX = gamma × interés abierto × 100 | — | [SqueezeMetrics, *Gamma Exposure (GEX)* (2016, revisado en 12-2017)](https://squeezemetrics.com/download/white_paper.pdf), leído el 26-11-2026 | C |
| Rebalanceo de los creadores de mercado y volatilidad | — | [Ni, Pearson, Poteshman y White (2021), *Does Option Trading Have a Pervasive Impact on Underlying Stock Prices?*, Review of Financial Studies 34(4), 1952–1986](https://ideas.repec.org/a/oup/rfinst/v34y2021i4p1952-1986..html), resumen leído el 26-11-2026 | A |
| Más de 60 futuros, 1974–2020, última media hora y cobertura de gamma | — | [Baltussen, Da, Lammers y Martens (2021), *Hedging Demand and Market Intraday Momentum*, Journal of Financial Economics 142(1), 377–403](https://econpapers.repec.org/RePEc:eee:jfinec:v:142:y:2021:i:1:p:377-403), resumen leído el 26-11-2026 | A |
| Impulso con gamma negativa, reversión con positiva, con una aproximación | — | [Barbon y Buraschi, *Gamma Fragility* (documento de trabajo, revisado el 19-03-2021)](https://abarbon.com/papers/gamma-fragility), resumen leído el 26-11-2026 | B |
| Precio cerca de un strike al vencimiento | `Stock price clustering on option expiration dates` | `docs/analysis/2026-11-patrones-de-manipulacion.md` | A |
| Única hipótesis de gamma con buena evidencia | `la única hipótesis con buena evidencia académica (Baltussen et al.)` | `docs/analysis/2026-11-patrones-de-manipulacion.md` | C |
| Strikes del S&P 500 cada 5 puntos | `los strikes van cada 5 puntos` | `docs/analysis/2026-11-patrones-de-manipulacion.md` | C |
| Última media hora antes de costos | `-0.010 / -0.013 / -0.031 R (t -0.83 / -1.19 / -2.62)` | `docs/analysis/registro-de-resultados.csv` `R026` | A |
| Última media hora con costos | `-2.0 % (t -11.0` | `docs/analysis/registro-de-resultados.csv` `R017` | A |
| *Open-Close* de Cboe: pago, uso interno, sin redistribuir | — | [Cboe DataShop, *Open-Close Volume Summary*](https://datashop.cboe.com/volume-summary-data), leída el 26-11-2026 | A |
| Términos de Cboe: guardar sin permiso escrito | `store either in hard copy or in an electronic retrieval system` | `docs/decisions/0012-tesoro-y-vix.md` | A |
| Un proveedor de datos nuevo va con ADR | `proveedor de datos` | `CLAUDE.md` | A |
