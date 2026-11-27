# 9.5 Casos reales del registro

> **Nivel 9 · Backtesting y sus trampas.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son
> resultados del pasado, con datos de futuros y costos del CFD: no dicen nada seguro del futuro.

## Objetivo

Leer, en nuestro propio registro de resultados, cómo apareció cada trampa del nivel: un período
favorable, un error de código, un placebo, una regla frágil, un modelo que no mejora, el costo y una
familia sobreajustada.

## La idea

El registro (`docs/analysis/registro-de-resultados.csv`) tiene **una fila por prueba**, con su
veredicto fijado **antes** de correr, y nunca se edita: si algo cambia, va una fila nueva que dice
contra cuál compara y si dio mejor, peor o igual. Siete casos:

**1. Un período favorable (R006 → R007).** Estrategia en uso en US100: +0,07 % por ventana con 8
ventanas; −6,1 % con 51. Fila nueva, `compara_con` R006, cambio **peor**. Trampa: pocas ventanas
(lección 9.4).

**2. Un error de código (R034 → R035).** La primera corrida de H5.1 (FVG) validaba desde 01-2013
en lugar de 02-2013. Se encontró, se corrigió y se volvió a correr: −0,0142 ATR pasó a −0,0233
ATR. Las dos no pasan, R034 queda en el registro marcada como inválida y **no suma** al N (la
reemplaza R035). Trampa: confiar en una corrida sin revisarla; la defensa es no borrar nada y decir
qué cambió.

**3. El placebo (R035 a R038).** ¿Un FVG de 15 m hace rebotar el precio más que un precio
cualquiera? Se comparó cada primer toque con **5 placebos emparejados** (mismo mes y franja, misma
llegada, tamaño y espera). Resultado: **−0,023 ATR** frente al placebo (p 0,963): reacciona un
poco **menos**. En 1 h, −0,063 ATR (R036). Filtrar la estrategia en uso con FVG tampoco mejora
(R037: −0,018 R, p 0,663) y la estrategia filtrada pierde contra comprar y mantener (R038). Trampa:
mirar «cuántas veces rebotó» sin compararlo con lo que hace un precio cualquiera.

**4. Una regla frágil (Bruni, programa 4).** La estrategia A de Gianluca Bruni, con reglas
medibles y congelada antes de correr:

| | Operaciones | Alfa por mes | p | Sharpe deflactado |
|---|---|---|---|---|
| H4.1, literal (cuerpo ≥ 15 puntos) | 539 | +0,123 % | 0,171 | 0,032 |
| H4.2, normalizada (cuerpo ≥ 0,35 ATR) | 1.359 | −0,397 % | 0,953 | 0,000 |
| H4.3, estrategia C (perfil de volumen) | 85 | −0,162 % | 0,994 | 0,000 |

Un cambio de parámetro «equivalente» (15 puntos frente a 0,35 ATR) le da vuelta el signo; la
literal gana casi todo en 2016–2020 y queda en cero desde 2021; H4.3 ni siquiera llega a las 100
operaciones. Ninguna pasa. Trampa: creer en una regla que depende de un número exacto (nivel 10,
caso Bruni completo).

**5. Un modelo que no mejora (ML v1 y v2, R010 a R013).** Un modelo de ML para predecir si una
operación toca su primer objetivo dio un AUC de **0,525** (0,5 = moneda), sin mejorar a la tabla
base B1; las tres variantes del v2 (tasas del Tesoro, VIX, análogos) dieron 0,502, 0,508 y 0,522.
Trampa: más variables y más complejidad no crean una señal que no está.

**6. El costo (R017 y R026).** El momentum de la última media hora da un t de **−11**, que parece
«una señal fuerte al revés». R026 lo explica: antes de costos está en cero (−0,010 a −0,031 R por
operación) y el costo fijo, repetido en unas 2.450 operaciones, hace un t enorme. Trampa: leer un t
grande sin separar antes y después de costos (nivel 8.1).

**7. Una familia sobreajustada (R040).** Cuatro cruces de EMA en el Laboratorio: todos con R
positivo, pero PBO **0,81** y Sharpe deflactado de 0,019 a 0,144 con N = 365. Es exploratorio: no
vota ni suma al N. Trampa: elegir «la mejor» de varias variantes (lección 9.3).

**El resumen honesto:** ninguna de las 13 hipótesis que votaron pasó, ningún modelo de ML superó a
su base y nada le ganó a comprar y mantener (+1,10 % por mes, R018). Saberlo **antes** de arriesgar
plata es para lo que sirve todo el método.

> **Probá antes de seguir:** ¿por qué R034 no se borró del registro, si estaba mal?
>
> <details><summary>Ver la respuesta</summary>
>
> Porque borrar filas es esconder pruebas: el registro tiene que mostrar todo lo que se corrió y
> qué cambió. R034 queda marcada como inválida, R035 dice contra qué compara y si dio mejor, peor o
> igual, y el N del Sharpe deflactado la cuenta una sola vez.
>
> </details>

## Interactivo

La línea de tiempo del registro: cada fila con su veredicto, `compara_con` y lo que cambió, y el
enlace a su informe.

## Ejercicio

Explicá cada caso en una frase, nombrando la trampa: R006 → R007, R034 → R035, R035, H4.1 y H4.2,
R010, R017 y R040.

<details><summary>Ver la solución</summary>

- R006 → R007: 8 ventanas eran un tramo favorable (pocas ventanas).
- R034 → R035: un error de código en el período, corregido sin borrar la fila vieja.
- R035: el FVG no reacciona más que un placebo emparejado.
- H4.1 y H4.2: un parámetro «equivalente» da vuelta el signo (fragilidad).
- R010: el ML no separa mejor que la tabla base (AUC ≈ 0,5).
- R017: el t −11 viene del costo, no de la señal.
- R040: elegir la mejor de una familia con PBO 0,81 es sobreajustar.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Registro de resultados**: todas las filas, con su veredicto, su comparación y su informe.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Una fila por prueba; las viejas no se editan | `Las filas viejas no se editan ni se borran.` | `docs/analysis/registro-de-resultados.md` | A |
| R007 compara con R006: peor | `de +0.07 % a -6.1 % por ventana` | `docs/analysis/registro-de-resultados.csv` `R007` | A |
| R034: error de código en el período | `con error de código: validación desde 01-2013` | `docs/analysis/registro-de-resultados.csv` `R034` | A |
| R035: corregido | `corregido el inicio del período (02-2013): la media pasa de -0.0142 a -0.0233 ATR` | `docs/analysis/registro-de-resultados.csv` `R035` | A |
| R034 no suma al N | `R034 (la corrida de H5.1 con un error de código) no suma` | `docs/analysis/2026-11-regla-presupuesto-global.md` | A |
| FVG frente al placebo | `-0.0233 ATR (t -1.80; p 0.963` | `docs/analysis/registro-de-resultados.csv` `R035` | A |
| 5 placebos emparejados | `5 placebos emparejados` | `docs/analysis/registro-de-resultados.csv` `R035` | A |
| FVG de 1 h | `-0.063 ATR` | `docs/analysis/registro-de-resultados.csv` `R036` | A |
| Filtro FVG en la estrategia en uso | `-0.018 R (p 0.663` | `docs/analysis/registro-de-resultados.csv` `R037` | A |
| Estrategia filtrada contra comprar y mantener | `pierde contra comprar y mantener en los dos índices` | `docs/analysis/registro-de-resultados.csv` `R038` | A |
| H4.1: operaciones, alfa, p y DSR | `539 operaciones` | `docs/analysis/registro-de-resultados.csv` `R029` | A |
| H4.1: alfa, p y DSR | `+0.123 % (p 0.171; DSR 0.032` | `docs/analysis/registro-de-resultados.csv` `R029` | A |
| H4.2: operaciones | `1359 operaciones` | `docs/analysis/registro-de-resultados.csv` `R030` | A |
| H4.2: alfa, p y DSR | `-0.397 % (p 0.953; DSR 0.000` | `docs/analysis/registro-de-resultados.csv` `R030` | A |
| H4.3: alfa, p, DSR y operaciones | `-0.162 % (p 0.994; DSR 0.000; -0.226 % / -0.057 %; 85 < 100)` | `docs/analysis/registro-de-resultados.csv` `R033` | A |
| H4.1 desde 2021 | `desde 2021 (428 operaciones) queda en` | `docs/analysis/2026-11-programa-4-h41-h42-resultados.md` | A |
| ML v1: AUC | `0.525 (-0.0006)` | `docs/analysis/registro-de-resultados.csv` `R010` | A |
| ML v2: tasas del Tesoro | `0.502 (+0.0020)` | `docs/analysis/registro-de-resultados.csv` `R011` | A |
| ML v2: VIX | `0.508 (+0.0008)` | `docs/analysis/registro-de-resultados.csv` `R012` | A |
| ML v2: análogos | `0.522 (-0.0002)` | `docs/analysis/registro-de-resultados.csv` `R013` | A |
| Momentum de la última media hora: t | `-2.0 % (t -11.0` | `docs/analysis/registro-de-resultados.csv` `R017` | A |
| Antes de costos, en cero | `-0.010 / -0.013 / -0.031 R` | `docs/analysis/registro-de-resultados.csv` `R026` | A |
| El t −11 viene del costo | `en ~2450 operaciones explica el t de -11` | `docs/analysis/registro-de-resultados.csv` `R026` | A |
| R040 | `+0.068 a +0.115 R; DSR 0.019 a 0.144; PBO 0.81` | `docs/analysis/registro-de-resultados.csv` `R040` | A |
| 13 hipótesis que votaron, ninguna pasa | `13 hipótesis` | `docs/analysis/2026-11-regla-presupuesto-global.md` | A |
| Comprar y mantener | `+1.0955 %` | `docs/analysis/registro-de-resultados.csv` `R018` | A |
