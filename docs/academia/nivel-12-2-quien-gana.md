# 12.2 Quién gana con el negocio

> **Nivel 12 · Cuentas fondeadas.** Educativo: no es consejo de inversión ni promete ganancias, y
> no recomienda ninguna firma. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)).
> Los números de los ejemplos son redondos, de práctica: no son las reglas de ninguna firma real.

## Objetivo

Calcular el **valor esperado** de un programa de fondeo con las fórmulas del motor: cuánto se paga
en cuotas hasta cobrar, cuánto se cobra y qué cobro medio hace falta para no perder.

## La idea

**1. De dónde sale la plata de la firma.** Entra por las cuotas de **todos** los intentos (también
los que fallan) y por las activaciones; sale por los cobros de los que cobran. En 2025, Topstep
publicó que se aprobó el **16,8 %** de sus evaluaciones: de cada 100 cuotas de evaluación, unas
**83** fueron de intentos que no pasaron. Nuestra investigación concluye que este negocio vive
sobre todo de las cuotas (es una inferencia, nivel C; la evidencia, en 12.3).

**2. Las fórmulas del motor.** Llamemos **q** a la probabilidad de **cobrar** en un intento (pasar
la evaluación **y** llegar al primer cobro; sale del Monte Carlo, lección 12.4) y **k** a los
intentos que estás dispuesto a pagar (5 por defecto):

- P(cobrar en ≤ k intentos) = 1 − (1 − q)^k
- intentos esperados = (1 − (1 − q)^k) ÷ q
- cuotas esperadas = intentos × (evaluación + activación × P(pasar la evaluación))
- **valor esperado** = P(cobrar) × (cobro medio + devolución) − cuotas esperadas

**Ejemplo** (sin activación ni devolución): q = 10 %, k = 5, cuota 100, cobro medio 800.

| Paso | Cuenta | Resultado |
|---|---|---|
| P(cobrar en ≤ 5) | 1 − 0,9⁵ | **41,0 %** |
| Intentos esperados | 0,40951 ÷ 0,10 | **4,10** |
| Cuotas esperadas | 4,0951 × 100 | **409,51** |
| Valor esperado | 0,40951 × 800 − 409,51 | **−81,90** |

**3. El atajo.** Sin activación ni devolución, las fórmulas se simplifican:

valor esperado = P(cobrar) × (cobro medio − cuota ÷ q)

El signo **no depende de k**: pagar más intentos no da vuelta un programa que pierde, solo agranda
la cuenta. Para no perder hace falta un cobro medio de al menos **cuota ÷ q**: en el ejemplo,
100 ÷ 0,10 = **1.000**, y el cobro medio era 800.

**4. Lo que achica el cobro medio.** El cobro es la ganancia de la etapa fondeada por el
**reparto** (por ejemplo, 80 % para vos), solo si llegaste a la ganancia y a los días mínimos sin
quemar la cuenta. La activación, si existe, se paga cada vez que pasás: con evaluación 100,
activación 150 y P(pasar) 25 %, cada intento cuesta 100 + 150 × 0,25 = **137,50**.

> **Probá antes de seguir:** q = 5 % y cuota 150. ¿Qué cobro medio hace falta para que el valor
> esperado no sea negativo?
>
> <details><summary>Ver la respuesta</summary>
>
> 150 ÷ 0,05 = **3.000**. Con una probabilidad baja de cobrar, el cobro medio tiene que ser muy
> grande para compensar todas las cuotas perdidas.
>
> </details>

## Interactivo

En el simulador de cuenta fondeada, el bloque de **economía**: intentos esperados, cuotas
esperadas, cobro medio y valor esperado, para tus operaciones y para operar al azar.

## Ejercicio

q = 20 %, k = 5, cuota 150, sin activación ni devolución, cobro medio 600. Calculá P(cobrar),
las cuotas esperadas y el valor esperado. ¿Qué cobro medio haría falta?

<details><summary>Ver la solución</summary>

P(cobrar en ≤ 5) = 1 − 0,8⁵ = **67,2 %**; intentos = 0,67232 ÷ 0,2 = 3,36; cuotas = 3,3616 × 150 =
**504,24**; valor esperado = 0,67232 × 600 − 504,24 = **−100,85**. Haría falta un cobro medio de
150 ÷ 0,2 = **750**.

</details>

## Examen

6 cálculos. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada todavía: el simulador se abre en la lección 12.4.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| P(cobrar en ≤ k intentos) | `P(cobrar en ≤ k intentos) = 1 − (1 − q)^k` | `docs/modules/propsim.md` | A |
| Intentos esperados | `intentos esperados = (1 − (1 − q)^k) / q` | `docs/modules/propsim.md` | A |
| Cuotas esperadas | `cuotas = intentos × (evaluación + activación × P(pasar))` | `docs/modules/propsim.md` | A |
| Valor esperado | `valor esperado = P(cobrar) × (cobro medio + devolución) − cuotas` | `docs/modules/propsim.md` | A |
| Intentos por defecto | `5` | `config/propsim.toml` `monte_carlo.default_max_attempts` | A |
| Evaluaciones de Topstep aprobadas en 2025 (autodeclarado) | 16,8 % | [Topstep, divulgación de riesgos (estadísticas de enero a diciembre de 2025)](https://www.topstep.com/risk-disclosure), leída el 26-11-2026 | C |
| El negocio vive sobre todo de las cuotas (inferencia) | `vive sobre todo de las cuotas de evaluación` | `docs/research/2026-11-terminal-pro-investigacion.md` | C |
