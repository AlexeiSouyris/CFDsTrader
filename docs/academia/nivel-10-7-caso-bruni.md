# 10.7 Caso Bruni: lo verificado y lo que no

> **Nivel 10 · Tu propia estrategia.** Educativo: no es consejo de inversión ni promete ganancias.
> Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Lo que muestra un
> trader son resultados pasados de otra persona, en otro mercado y con otro riesgo.

## Objetivo

Leer un caso completo separando **lo verificado** (títulos en una tabla oficial) de **lo no
verificado** (backtests y pagos), y ver qué dieron sus reglas con nuestro método.

## La idea

Gianluca Bruni es un trader de forex que el dueño trajo como referencia (#561). El caso sirve porque mezcla
las tres cosas que aparecen siempre: un logro real, afirmaciones sin auditar y reglas que se pueden
probar.

**1. Lo verificado** (tabla oficial de la World Cup Trading Championships, leída por nuestro
informe el 25-11-2026):

- ganó el campeonato **trimestral** de forex tres veces, con dinero real: 2025 Q4 (58,6 %), 2026 Q2
  (104,8 %) y 2026 Q3 (245,2 %);
- el de 2026 Q4 lo iba ganando, pero **no había terminado**, y no ganó el campeonato **anual**;
- en esa competencia el apalancamiento llega a 200:1 en forex.

**2. Lo que no se pudo verificar:**

- sus backtests («15 años y más de 5.000 operaciones», 147 R en 2 años, Sharpe 2,2): son capturas
  de pantalla, sin un informe auditado;
- lo que dice haber cobrado de empresas de fondeo (da cifras distintas en videos distintos);
- con cuánto capital compite, cuántas cuentas usa y su peor caída real.

**3. Las alertas** (todas en el informe, con sus fuentes):

- **Conflictos de interés:** empresas de fondeo que lo patrocinan, un código de descuento a su
  nombre y cursos pagos en el pasado. Gana más si más gente compra desafíos.
- **La competencia premia el riesgo:** gana el mayor % en tres meses, sin límite de caída. Eso
  favorece arriesgar mucho: según él, usa de 0,5 % a 4 % por operación con fracciones de Kelly.
  Con 4 % por operación, 8 pérdidas seguidas se llevan más del 25 % de la cuenta (nivel 8). Una
  estrategia que gana un torneo así puede ser mala para una cuenta que tiene que durar años.
- **Supervivencia y campo chico:** solo se publica el top y los intentos fallidos no se ven (nivel
  9.2). En 2026 Q2, del 3.º al 5.º puesto hicieron entre 2,7 % y 3,3 %: parece que compiten pocos.
- **Falacia del jugador:** dice que después de 8 pérdidas la siguiente tiene más chances de ganar
  y sube el riesgo ahí. Sin muchos datos que lo prueben, es la falacia del jugador, y subir el
  riesgo justo en una racha es lo que más daño hace (nivel 8.2).

**4. Lo que dieron sus reglas con nuestro método** (programa 4, reglas congeladas antes de correr,
US100, 2013–2025):

| Hipótesis | Qué es | Resultado |
|---|---|---|
| H4.1 (R029) | estrategia A literal: dos velas de 15 m con cuerpo ≥ 15 puntos a favor de la estructura | +0,123 % por mes, p 0,171, Sharpe deflactado 0,032: **no pasa** |
| H4.2 (R030) | la misma, con cuerpo ≥ 0,35 × ATR | −0,397 % por mes: **no pasa** |
| H4.3 (R033) | estrategia C con perfil de volumen | −0,162 % por mes y solo 85 operaciones: **no pasa** |

**5. Qué nos llevamos.** Lo que lo diferencia no es un indicador secreto sino un **proceso** que
ya es el nuestro: reglas objetivas, muchas operaciones antes de confiar, costos reales, fuera de
muestra y comparación con comprar y mantener. Lo que **no** adoptamos: el tamaño variable (el
riesgo del proyecto es 1 % fijo). Ganar un torneo de tres meses con riesgo alto y tener una ventaja
que sobreviva años con riesgo chico son preguntas distintas.

> **Probá antes de seguir:** «Ganó tres campeonatos, así que sus reglas funcionan». ¿Qué está mal
> en esa frase?
>
> <details><summary>Ver la respuesta</summary>
>
> Mezcla lo verificado (los títulos) con lo no verificado (que sean **esas** reglas las que ganan,
> y que ganen con riesgo normal). Los títulos son con mucho riesgo y en otro mercado; sus reglas
> medibles, probadas con nuestro método, no pasaron. Las dos cosas pueden ser ciertas a la vez.
>
> </details>

## Interactivo

El informe completo (`docs/research/2026-11-gianluca-bruni.md`) con cada afirmación marcada como
verificada, no verificada o inferida, y los resultados de H4.1, H4.2 y H4.3 al lado.

## Ejercicio

Separá en dos columnas, verificado y no verificado: «3 títulos trimestrales», «Sharpe 2,2», «más
de USD 1 M gestionado en 6 firmas», «58,6 % en 2025 Q4», «15 años de backtest».

<details><summary>Ver la solución</summary>

- **Verificado:** 3 títulos trimestrales; 58,6 % en 2025 Q4 (tabla oficial).
- **No verificado:** Sharpe 2,2; USD 1 M en 6 firmas; 15 años de backtest (capturas y relatos, sin
  auditar).

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada nuevo. Con esta lección termina el nivel 10.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Tres títulos trimestrales | `tres veces (2025 Q4, 2026 Q2 y 2026 Q3)` | `docs/research/2026-11-gianluca-bruni.md` | A |
| 2025 Q4 | `58,6 %` | `docs/research/2026-11-gianluca-bruni.md` | A |
| 2026 Q2 | `104,8 %` | `docs/research/2026-11-gianluca-bruni.md` | A |
| 2026 Q3 | `245,2 %` | `docs/research/2026-11-gianluca-bruni.md` | A |
| No ganó el anual | `No ganó el campeonato` | `docs/research/2026-11-gianluca-bruni.md` | A |
| Apalancamiento de la competencia | `200:1` | `docs/research/2026-11-gianluca-bruni.md` | A |
| Backtests sin auditar | `Son capturas de pantalla, no hay un informe auditado` | `docs/research/2026-11-gianluca-bruni.md` | C |
| Cobros de empresas de fondeo, sin verificar | `da las dos cifras en videos distintos` | `docs/research/2026-11-gianluca-bruni.md` | C |
| Conflictos de interés | `un código de descuento a su nombre` | `docs/research/2026-11-gianluca-bruni.md` | C |
| Tamaño variable con Kelly | `de 0,5 % a 4 % por operación con una fracción de Kelly` | `docs/research/2026-11-gianluca-bruni.md` | C |
| 8 pérdidas al 4 % | `se lleva más del 25 % de la cuenta` | `docs/research/2026-11-gianluca-bruni.md` | A |
| Campo chico en 2026 Q2 | `entre 2,7 % y 3,3 %` | `docs/research/2026-11-gianluca-bruni.md` | A |
| H4.1 | `+0.123 % (p 0.171; DSR 0.032` | `docs/analysis/registro-de-resultados.csv` `R029` | A |
| H4.2 | `-0.397 % (p 0.953` | `docs/analysis/registro-de-resultados.csv` `R030` | A |
| H4.3 | `85 < 100` | `docs/analysis/registro-de-resultados.csv` `R033` | A |
| Riesgo del proyecto | 1 % (`1`) | `config/risk.toml` `limits.risk_per_trade_pct` | A |
