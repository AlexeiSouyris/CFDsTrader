# 2.2 Financiación nocturna (swap)

> **Nivel 2 · Costos reales.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)).

## Objetivo

Calcular el costo de "dormir" con una posición abierta: la [financiación nocturna](../GLOSARIO.md#swap)
de un CFD de índice.

## La idea

Un CFD al contado no vence. A cambio, **cada noche** que la posición sigue abierta el broker cobra
(o, a veces, paga) un interés sobre el nominal. La fórmula de IG para índices es:

```text
costo = noches × precio de cierre × tamaño × tasa / 360
largo: tasa = referencia + 3 %        (paga)
corto: tasa = 3 % − referencia        (paga si la referencia es menor que 3 %; si es mayor, cobra)
```

- **Referencia:** el tipo de interés a un día de la moneda del índice. Para US500, US100 y US30, IG
  confirmó que usa el **SOFR** (EE. UU.); para el DE40, el **€STR** (zona euro).
- **3 %:** la tasa administrativa de IG para CFD de índices.
- **360:** la base de días de IG para todos los índices salvo el FTSE 100 (que usa 365).
- **Cuándo:** en cada corte de las **23:00, hora de Madrid**, en que la posición sigue abierta. El
  motor cuenta cada corte que la posición cruza, también los del sábado y el domingo.

Los valores de referencia en nuestra config hoy: SOFR **3,94 %** (Fed de Nueva York, valor del
24-11-2026) y €STR **2,441 %** (BCE, valor del 19-11-2026). Son el respaldo: el backtest y el paper
usan la serie diaria guardada cuando existe, con la tasa que estaba publicada **cada noche**.

**El oro es distinto:** usa la base de la curva de futuros y no un tipo de referencia, y todavía le
faltan datos (por eso no opera ni en paper).

> **Probá antes de seguir:** con el SOFR en 3,94 %, ¿un corto en US500 paga o cobra?
>
> <details><summary>Ver la respuesta</summary>
>
> Cobra un poco: 3 % − 3,94 % = −0,94 % anual, negativo, así que el costo es negativo. Si el SOFR
> bajara de 3 %, el corto empezaría a pagar.
>
> </details>

## Interactivo

Un deslizador de noches: elegís instrumento, lado, tamaño y fecha, y el motor calcula con la serie
real de tipos de referencia del día elegido.

## Ejemplo resuelto

**Con números redondos.** Precio 6.000, tasa 4 % + 3 % = 7 %:
6.000 × 7 % ÷ 360 ≈ **1,17 puntos por noche** y por unidad.

**Con nuestra config** (precios de ejemplo, redondos):

| Caso | Tasa anual | Cuenta | Por noche |
|---|---|---|---|
| Largo US500 en 6.000, tamaño 1 | 3,94 % + 3 % = 6,94 % | 6.000 × 6,94 % ÷ 360 | **1,16 €** (paga) |
| Corto US500 en 6.000, tamaño 1 | 3 % − 3,94 % = −0,94 % | 6.000 × −0,94 % ÷ 360 | **−0,16 €** (cobra) |
| Largo DE40 en 24.000, tamaño 1 | 2,441 % + 3 % = 5,441 % | 24.000 × 5,441 % ÷ 360 | **3,63 €** (paga) |
| Corto DE40 en 24.000, tamaño 1 | 3 % − 2,441 % = 0,559 % | 24.000 × 0,559 % ÷ 360 | **0,37 €** (paga) |

Tres noches del largo US500: 3 × 1,1567 ≈ **3,47 €**. Un año entero (365 noches con base 360) son
unos **7,0 %** del nominal: mantener un largo mucho tiempo cuesta.

En nuestra estrategia de 15 minutos las posiciones duran poco: si a las 32 velas (8 horas de
mercado) no llegaron a +1 R se cierran, y el viernes se cierra todo antes del fin de semana. Por eso
la financiación fue solo un 5 a 10 % de sus costos. Para quien mantiene posiciones semanas, pasa a
ser un costo grande.

## Ejercicio

Un largo de tamaño 2 en US100 a 25.000 (precio de ejemplo) queda abierto 5 noches con el SOFR en
3,94 %. ¿Cuánto paga?

<details><summary>Ver la solución</summary>

5 × 25.000 × 2 × 6,94 % ÷ 360 = 17.350 ÷ 360 ≈ **48,19 €**.

</details>

## Examen

6 preguntas (5 de cálculo y lectura, 1 de concepto). Para aprobar hace falta el 80 % (umbral
provisorio).

## Desbloquea

Nada nuevo: la financiación aparece en la columna de costos que abrió la lección 2.1.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Tasa administrativa de IG en US500 | 3 % (`3`) | `config/instruments.toml` `instruments.US500.funding_admin_fee_pct` (ig.com/es, CFD de índices) | A |
| Tasa administrativa de IG en DE40 | 3 % (`3`) | `config/instruments.toml` `instruments.DE40.funding_admin_fee_pct` | A |
| Base de días de los índices | 360 (`360`) | `config/instruments.toml` `instruments.US500.funding_day_count` | A |
| Referencia de US500 | `SOFR` | `config/instruments.toml` `instruments.US500.funding_benchmark` (chat de IG, #655) | A |
| Referencia de DE40 | `ESTR` | `config/instruments.toml` `instruments.DE40.funding_benchmark` | A |
| SOFR (valor del 24-11-2026) | 3,94 % (`3.94`) | `config/instruments.toml` `funding_benchmarks.SOFR` (Fed de Nueva York) | A |
| €STR (valor del 19-11-2026) | 2,441 % (`2.441`) | `config/instruments.toml` `funding_benchmarks.ESTR` (BCE) | A |
| Serie diaria cuando existe; la constante es el respaldo | `serie diaria guardada (config/reference_rates.toml, #79) cuando existe` | `config/instruments.toml` (comentario de funding_benchmarks) | A |
| Hora del corte | 23:00 (`23:00`) | `config/instruments.toml` `account.funding_cutoff` | A |
| Zona horaria del corte | `Europe/Madrid` | `config/instruments.toml` `account.funding_cutoff_timezone` | A |
| Corto: tasa = tasa adm. − referencia | `corto: tasa = adm. − referencia (puede cobrar)` | `engine/crates/backtest/src/costs.rs` | A |
| La financiación fue el 5–10 % de los costos de la estrategia | `El swap es el 5–10 % de los costos` | `docs/analysis/2026-11-diagnostico-us-futuros.md` | A |
| Salida por tiempo de la estrategia | 32 velas (`32`) | `config/backtest.toml` `exits.time_exit_bars` | A |
| Viernes: se cierra todo antes del cierre de la sesión | `Viernes: se cierra todo` | `docs/spec/fase-1-especificacion.md` §8 | A |
| Oro: base de la curva de futuros, con datos pendientes | `commodity_basis` | `config/instruments.toml` `instruments.XAUUSD.funding_model` (ADR 0013) | A |
| Fórmula, divisor 360 (365 solo FTSE 100), 3 % y corte de las 23:00 CE(S)T | — | [IG España, "Nuestros costes", financiación nocturna de CFD de índices](https://www.ig.com/es/nuestros-costes), leído el 26-11-2026 | A |
| SOFR | — | [Federal Reserve Bank of New York, SOFR](https://www.newyorkfed.org/markets/reference-rates/sofr) | A |
| €STR | — | [BCE, euro short-term rate](https://www.ecb.europa.eu/stats/financial_markets_and_interest_rates/euro_short-term_rate/html/index.en.html) | A |
