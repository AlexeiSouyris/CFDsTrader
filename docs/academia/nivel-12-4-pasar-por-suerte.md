# 12.4 Pasar por suerte: el simulador

> **Nivel 12 · Cuentas fondeadas.** Educativo: no es consejo de inversión ni promete ganancias, y
> no recomienda ninguna firma. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)).
> Los números de los ejemplos son redondos, de práctica: no son las reglas de ninguna firma real.

## Objetivo

Usar el simulador de cuentas fondeadas para ver con qué probabilidad unas operaciones **cobran,
queman la cuenta o se quedan sin tiempo**, cuánto valen **después de las cuotas** y si le ganan a
operar **al azar**, y llegar a la conclusión honesta.

## La idea

**1. Sin ventaja también se pasa.** Con operaciones de esperanza 0 y ganancias y pérdidas del mismo
tamaño (sin costos, sin pérdida diaria y sin límite de días), la probabilidad de llegar al objetivo
antes que a la caída máxima fija es, aproximadamente:

P(pasar) ≈ caída máxima ÷ (objetivo + caída máxima)

Con objetivo 8 % y caída 10 %: 10 ÷ 18 ≈ **56 %**. Más de la mitad pasa **sin ninguna ventaja**.
Por eso pasar una evaluación no prueba nada sobre tu método: juzgar la decisión por el resultado
es el *resulting* de la lección 8.4. Es la misma idea de la Academia y de la propuesta de
[Trofeos de proceso](../design/2026-11-trofeos-reglas.md): premian el proceso, nunca la ganancia.

**2. Qué hace el simulador** (motor `cfds-propsim`; la UI solo arma el pedido y muestra):

- **Operaciones:** una lista de R, las de un informe del Laboratorio o un modelo (win rate y R medio
  ganador y perdedor, como el simulador de varianza).
- **Cada camino es un intento** (una cuota). Con una lista o un informe, los días reales se
  remuestrean en bloques de **5** días operados (con el modelo, se sortean), hasta **250** días por
  intento; por defecto **5.000** caminos (el error de una probabilidad queda en 0,7 % o menos) y
  semilla fija: el mismo pedido da el mismo resultado.
- **Resultado por intento:** pasar la evaluación, cobrar, quemar la cuenta, quedarse sin tiempo o
  sin terminar, cada uno con su intervalo de Wilson del 95 %; y la economía de la lección 12.2 con
  hasta **5** intentos.
- **Comparación con el azar:** la misma semilla con tus operaciones **centradas en 0** (esperanza
  0). Solo dice que «le ganás al azar» si los intervalos **no se tocan**.
- **Riesgo:** se puede simular hasta **10 %** por operación, a propósito más que el **1 %** de
  nuestra config, para **ver** qué pasa con riesgos altos.
- La caída que sigue al máximo **intradía** no se simula (12.1): con esa regla, pasar es un tope
  superior.

**3. Cómo leerlo.** Tres preguntas, en este orden:

1. ¿Tu probabilidad de cobrar es **claramente** mayor que la del azar, o los intervalos se tocan?
2. ¿El valor esperado **después de cuotas** es positivo?
3. ¿De dónde salen tus operaciones? Si vienen de un backtest exploratorio o de un replay corto, el
   simulador hereda su sobreajuste y su suerte (nivel 9).

**4. La conclusión honesta.**

- Sin una ventaja **verificada** (una regla pre-registrada que pasó fuera de muestra, nivel 10), tus
  operaciones reales no tienen esperanza 0: tienen esperanza **negativa**, porque los costos se pagan
  siempre. Ejemplo nuestro: la última media hora dio cero o algo negativo **antes** de costos (R026)
  y −2,0 % por mes **con** los costos del CFD (R017).
- Con esperanza negativa, cada intento pasa **menos** que con esperanza 0 y las cuotas se suman en
  cada intento: lo esperable es un **valor esperado negativo**.
- Si el simulador te da positivo con esperanza 0, atacá primero los supuestos: el cobro medio, la
  regla intradía que no simulamos, que la firma pague y no cambie sus reglas, y que la cuenta
  «fondeada» puede ser simulada (en Topstep, el 0,71 % de las personas con *Express Funded* fue
  llamada a una cuenta real en 2025). La firma pone sus reglas para que, en conjunto, las cuotas
  paguen los cobros (inferencia).
- «Con muchos desafíos se gana sin ventaja» (12.3) no está verificado: el simulador es la manera de
  ponerlo a prueba con reglas reales, **y con los costos**.

Una cuenta fondeada no crea ventaja: cambia quién pone el capital y cuánto podés perder (la cuota).
Si no hay ventaja medida, lo que se compra es una chance de que salga bien por suerte.

> **Probá antes de seguir:** objetivo 10 %, caída fija 5 %, esperanza 0. ¿Cuántos de cada 100
> intentos pasan, más o menos?
>
> <details><summary>Ver la respuesta</summary>
>
> 5 ÷ 15 ≈ **33**. Con la caída más chica que el objetivo, pasar es menos probable, aun sin costos.
>
> </details>

## Interactivo

El **simulador de cuenta fondeada** (Laboratorio): cargá las reglas en la plantilla genérica,
usá tus operaciones o las de un informe y miralas al lado de las mismas operaciones con esperanza 0.

## Ejercicio

1. Objetivo 6 %, caída fija 6 %, esperanza 0: ¿qué probabilidad aproximada de pasar?
2. El simulador dice: tus operaciones cobran en el 18 % de los intentos (14–22 %); operando al
   azar, en el 15 % (12–19 %). ¿Le ganás al azar?

<details><summary>Ver la solución</summary>

1. 6 ÷ 12 = **50 %**.
2. **No se distingue**: los intervalos se tocan (14–19 % es común a los dos). La diferencia puede ser
   suerte.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Simulador de cuentas fondeadas** (Laboratorio): reglas de una evaluación, tus operaciones o las
  de un informe, probabilidad de cobrar o fallar, valor esperado después de cuotas y la comparación
  con operar al azar. Con esta lección termina la Academia.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Caminos por defecto | `5000` | `config/propsim.toml` `monte_carlo.default_paths` | A |
| Error de una probabilidad con 5.000 caminos | `≤ 0,7 %` | `config/propsim.toml` | A |
| Bloques de días operados | `5` | `config/propsim.toml` `monte_carlo.default_block_len_days` | A |
| Días operados por intento | `250` | `config/propsim.toml` `monte_carlo.default_horizon_days` | A |
| Intentos para el valor esperado | `5` | `config/propsim.toml` `monte_carlo.default_max_attempts` | A |
| Semilla fija | `20261126` | `config/propsim.toml` `monte_carlo.default_seed` | A |
| Intervalos de Wilson del 95 % | `0.95` | `config/propsim.toml` `monte_carlo.confidence` | A |
| Riesgo máximo que se puede simular | 10 % (`10`) | `config/propsim.toml` `limits.max_risk_pct` | A |
| Riesgo del proyecto | 1 % (`1`) | `config/risk.toml` `limits.risk_per_trade_pct` | A |
| Resultados por intento | `pasar la evaluación, cobrar, quemar la cuenta, sin tiempo, sin terminar` | `docs/modules/propsim.md` | A |
| Comparación con esperanza 0 | `la misma semilla con la lista centrada en 0` | `docs/modules/propsim.md` | A |
| Le gana al azar solo si los intervalos no se tocan | `solo si los intervalos no se tocan` | `docs/modules/propsim.md` | A |
| Operaciones de un informe del Laboratorio | `remuestreen los R reales del informe` | `docs/modules/lab.md` | A |
| Última media hora antes de costos | `-0.010 / -0.013 / -0.031 R` | `docs/analysis/registro-de-resultados.csv` `R026` | A |
| Última media hora con costos | `-2.0 % (t -11.0` | `docs/analysis/registro-de-resultados.csv` `R017` | A |
| Topstep 2025: de *Express Funded* a cuenta real (autodeclarado) | 0,71 % | [Topstep, divulgación de riesgos](https://www.topstep.com/risk-disclosure), leída el 26-11-2026 | C |
| Los trofeos premian proceso | `Premian proceso, nunca ganancia` | `docs/design/2026-11-trofeos-reglas.md` | A |
| *Resulting* | — | Annie Duke (2018), *Thinking in Bets*, Portfolio/Penguin; definición en [la entrevista de Nautilus (2017)](https://nautil.us/the-resulting-fallacy-is-ruining-your-decisions-236901), leída el 26-11-2026 | B |
| «Se gana sin ventaja» con muchos desafíos (sin verificar) | `se gana aunque la estrategia no tenga ventaja` | `docs/research/2026-11-gianluca-bruni.md` | C |
