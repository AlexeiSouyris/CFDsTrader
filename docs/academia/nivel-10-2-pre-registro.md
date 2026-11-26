# 10.2 Pre-registro, filtro de borde bruto y presupuesto de alfa

> **Nivel 10 · Tu propia estrategia.** Educativo: no es consejo de inversión ni promete ganancias.
> Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Una hipótesis es una
> pregunta: lo normal es que la respuesta sea «no».

## Objetivo

Congelar la regla, los criterios y el período **antes** de ver resultados, pasar el **filtro de
borde bruto** (k = 2) y pagar la apuesta del **presupuesto global de alfa**.

## La idea

**1. Por qué pre-registrar.** Harvey (2017), en su discurso como presidente de la American
Finance Association, describe el problema: las revistas quieren resultados «significativos», hay
pruebas que no se informan, no se corrige por muchas pruebas y se hace *p-hacking*; propone,
entre otras cosas, aceptar informes registrados **antes** de tener los datos. El pre-registro es
eso, hecho en casa: escribir y congelar todo lo que se va a decidir **antes** de mirar un solo
resultado.

**2. Qué tiene un pre-registro** (los nuestros: programas 4, 5 y Nasdaq):

- la hipótesis, la regla completa (lección 10.1) y los datos;
- los **criterios de voto**: los cinco del alfa contra comprar y mantener — alfa con p < nivel,
  Sharpe deflactado ≥ 0,95 con el N al congelar, ventaja positiva en las **dos mitades**, alfa
  positivo con **costos × 1,5** y que **agregue** a comprar y mantener;
- la lista de lo **prohibido después de ver resultados**: cambiar la regla, los costos, el
  período, las mitades, pasar a otro timeframe «porque dio mejor», mirar el fuera de muestra o
  volver a correr con otra regla y presentar la mejor. Cualquiera de esas cosas es una **prueba
  nueva** y se paga;
- los **errores de código** sí se corrigen: se vuelve a correr y se informa con fecha, sin borrar
  el resultado anterior (R034 → R035);
- el **hash**: el archivo se congela con su SHA-256 en un `.sha256` al lado. El programa 4 se
  congeló el 25-11-2026 a las 03:59 UTC, antes de correr nada, con SHA-256 que empieza en
  `973f7fd2`. Si alguien cambia una coma, el hash ya no coincide.

**3. El filtro de borde bruto (antes de gastar).** Antes de congelar, la hipótesis tiene que
mostrar, **sin mirar resultados**, que su ventaja bruta esperada por operación cubre el costo con
margen:

E_bruto ≥ k × C, con **k = 2**

- **C** = spread + slippage + comisión + financiación de los largos, todo de `config/`. Intradía
  en la sesión de contado: US100 **2,12** puntos, US500 **1,10**, US30 **4,64**. Una noche en
  largo de un US100 en 20.000 cuesta 3,86 puntos: casi el doble del spread.
- **Por qué 2:** las anomalías publicadas pierden en promedio un 26 % fuera de la muestra del
  artículo y un 58 % después de publicadas (McLean y Pontiff, 2016). Con k = 2, aunque el borde se
  reduzca a la mitad, cubre los costos.
- **E_bruto previo:** de la literatura (con cita) o IC × σ_h, con σ_h el desvío del retorno en el
  horizonte de la operación (un dato que no usa la regla) e IC = 0,05 si no hay cita. Con σ_h = 40
  puntos: 0,05 × 40 = 2 puntos, menos que 2 × 2,12 = 4,24. **No entra.**
- No vale usar el P/L o el win rate de la propia regla. Si no pasa, la hipótesis no se congela ni
  paga: queda anotada como «descartada por el filtro».

**4. El presupuesto global de alfa** (aprobado por el dueño el 25-11-2026). Antes, cada programa
abría su propio 5 %: con 13 hipótesis se gastó 18,75 % y, a la larga, algo iba a «pasar» por azar.
Ahora hay **una sola riqueza W para todo el proyecto** (alfa-inversión, Foster y Stine, 2008):

- arranca en **W = 4,75 %**;
- cada hipótesis apuesta α_j = mín(1,25 %, W ÷ (1 + W)) y **paga α_j ÷ (1 − α_j) al congelarse**,
  pase o no, y aunque después no se corra: 1,25 % cuesta **1,2658 %**;
- solo si pasa **y el fuera de muestra la confirma**, devuelve un premio ω = 5 %;
- lo descriptivo (que no vota) no gasta.

Sin descubrimientos, alcanza para unas 4 hipótesis: 4,75 % → 3,48 % → 2,22 % → 0,95 % → 0. Cada
pre-registro trae W antes y después, su α_j, el filtro con sus números y el N al congelar, y el PR
que lo congela agrega su fila al libro de `registro-de-resultados.md`; el CI verifica la cuenta.
Hoy el libro tiene solo la fila de inicio: **ninguna hipótesis gastó todavía**.

> **Probá antes de seguir:** ¿por qué una hipótesis paga aunque después no se corra?
>
> <details><summary>Ver la respuesta</summary>
>
> Porque si se pagara solo al correr, se podría congelar muchas, mirar algo de los datos y correr
> solo las que «prometen»: otra forma de elegir después de ver. Pagar al congelar hace que cada
> apuesta se piense antes.
>
> </details>

## Interactivo

La plantilla de pre-registro (hipótesis, regla, criterios, prohibidos, filtro de borde bruto, W
antes y después, α_j y N) y el libro del presupuesto global con su única fila de hoy.

## Ejercicio

Pre-registrá una idea propia en papel con la plantilla. Calculá su C con los valores de la
lección, estimá E_bruto (con una cita o con IC × σ_h) y decidí si pasa el filtro. Si pasa,
calculá cuánto pagaría y cuánto quedaría en W.

<details><summary>Ver la solución</summary>

Ejemplo: una idea intradía en US500 con σ_h = 25 puntos y sin cita: E_bruto ≈ 0,05 × 25 = 1,25
puntos, menos que 2 × 1,10 = 2,20. **No pasa el filtro**: no se congela y no paga. Para pasar haría
falta un IC de al menos 2,20 ÷ 25 = 0,088 con una cita que lo respalde, o un horizonte más largo
con más σ_h por operación. Si pasara con α_j = 1,25 %: paga 1,2658 % y W queda en 3,4842 %.

</details>

## Examen

6 preguntas (cálculos y conceptos). Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada nuevo: el Laboratorio se abre en la lección 10.3.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Programa 4: congelado antes de correr, con su hash | `congelado el 25-11-2026 a las 03:59 UTC` | `docs/analysis/2026-11-programa-4-h41-h42-resultados.md` | A |
| Hash del programa 4 | `973f7fd2ddcaf4a333eca8e8d179f29cee0900a36e10a4c8be5f084cceb47460` | `docs/analysis/2026-11-programa-4-preregistro.md.sha256` | A |
| Los cinco criterios del alfa | `Los cinco criterios del alfa contra comprar y mantener` | `engine/crates/backtest/examples/common/alpha_verdict.rs` | A |
| Criterio 5: agregar a comprar y mantener | `Agrega a comprar y mantener:` | `docs/analysis/2026-11-programa-4-preregistro.md` | A |
| Prohibido: volver a correr y presentar la mejor | `volver a correr con otra regla y presentar la mejor` | `docs/analysis/2026-11-programa-5-fvg-preregistro.md` | A |
| Errores de código: se corrigen sin borrar | `El anterior no se borra` | `docs/analysis/2026-11-programa-5-fvg-preregistro.md` | A |
| Factor k del filtro | 2 (`2`) | `config/research.toml` `alpha_budget.edge_cost_factor` | A |
| IC previo sin cita | `0.05` | `config/research.toml` `alpha_budget.default_ic` | A |
| C intradía: US100 | `2,12 puntos` | `docs/analysis/2026-11-regla-presupuesto-global.md` | A |
| C intradía: US500 | `1,10 puntos` | `docs/analysis/2026-11-regla-presupuesto-global.md` | A |
| C intradía: US30 | `4,64 puntos` | `docs/analysis/2026-11-regla-presupuesto-global.md` | A |
| Una noche en largo de un US100 en 20.000 | `3,86 puntos por noche` | `docs/analysis/2026-11-regla-presupuesto-global.md` | A |
| Slippage: fracción del spread | `0.1` | `config/backtest.toml` `costs.slippage_spread_fraction` | A |
| Decaimiento de las anomalías publicadas | `un 26 % fuera de la muestra del artículo y un 58 % después de publicadas` | `docs/analysis/2026-11-regla-presupuesto-global.md` | A |
| Riqueza inicial W | 4,75 % (`0.0475`) | `config/research.toml` `alpha_budget.initial_wealth` | A |
| Apuesta máxima | 1,25 % (`0.0125`) | `config/research.toml` `alpha_budget.max_bet` | A |
| Premio solo si confirma | 5 % (`0.05`) | `config/research.toml` `alpha_budget.payout` | A |
| Lo gastado antes | `18,75 %` | `docs/analysis/2026-11-regla-presupuesto-global.md` | A |
| Cuatro hipótesis sin descubrimientos | `4,75 % → 3,48 % → 2,22 % → 0,95 %` | `docs/analysis/registro-de-resultados.md` | A |
| El libro tiene solo la fila de inicio | `W(0): inicio de la época` | `docs/analysis/registro-de-resultados.md` | A |
| Alfa-inversión (método) | `Foster y Stine, 2008` | `docs/analysis/2026-11-regla-presupuesto-global.md` | A |
| Pruebas sin informar, *p-hacking* e informes registrados | — | [Harvey (2017), *Presidential Address: The Scientific Outlook in Financial Economics*, Journal of Finance 72(4), 1399–1440](https://doi.org/10.1111/jofi.12530), texto leído el 26-11-2026 | A |
