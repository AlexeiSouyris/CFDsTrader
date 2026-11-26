# 12.1 Cómo funcionan las cuentas fondeadas

> **Nivel 12 · Cuentas fondeadas.** Educativo: no es consejo de inversión ni promete ganancias, y
> no recomienda ninguna firma. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)).
> Los números de los ejemplos son redondos, de práctica: no son las reglas de ninguna firma real.

## Objetivo

Conocer las reglas de una evaluación de una *prop firm* (fases, objetivo, pérdida diaria, caída
máxima fija o que sigue al máximo, días, consistencia, cobro, reparto y cuotas) con la plantilla
genérica del simulador, y entender por qué la caída que sigue al máximo es la más traicionera.

## La idea

**1. El trato.** Una firma de fondeo (*prop firm*) cobra una **cuota** por una evaluación (un
«desafío»). Si llegás al objetivo de ganancia sin romper ninguno de sus límites, pasás a una cuenta
«fondeada» y cobrás una parte de lo que ganes ahí. Ojo con la palabra «fondeada»: puede seguir
siendo un entorno **simulado**. Por ejemplo, Topstep dice en sus preguntas frecuentes que en su
cuenta *Express Funded* se sigue operando en mercados simulados. Antes de creer cualquier
cosa, leé el reglamento vigente de la firma: cambian sus reglas seguido.

**2. Las reglas, una por una** (la forma de `prop_rules.v1`; en el simulador la plantilla
genérica las trae **en blanco**, porque no se inventan reglas de firmas reales):

| Regla | Qué es | Por qué importa |
|---|---|---|
| Tamaño y moneda | El capital nominal de la cuenta (por ejemplo, 50.000). | Todos los % se miden contra él. |
| Corte del día | Zona horaria y hora en que la firma cierra «el día». | Define qué es la pérdida de un día. |
| Fases | Una o dos evaluaciones, cada una con su **objetivo de ganancia** (%), días mínimos, días máximos y, a veces, una **regla de consistencia** (el mejor día no puede pasar de un % de la ganancia total). | Más fases = más veces que podés fallar. |
| Pérdida diaria | Un % que no podés perder en un día, medido desde el **tamaño** de la cuenta o desde el **saldo al inicio del día**. | Una mala mañana te saca. |
| Caída máxima | Fija (desde el saldo inicial), que **sigue al máximo al cierre del día** o que **sigue al máximo intradía**; con o sin «traba» en el saldo inicial. | Si sigue al máximo, el margen se achica con cada ganancia, aunque no la cierres. |
| Etapa fondeada | Días y ganancia mínima para el primer cobro, **reparto** (qué % es tuyo) y, a veces, consistencia. | Pasar no es cobrar. |
| Cuotas | La de la evaluación, una de **activación** al pasar, y si la de evaluación se devuelve con el primer cobro. | Se paga en **cada** intento. |

**3. La caída que sigue al máximo.** Con una caída fija de 2.000 en una cuenta de 50.000, el piso
queda en 48.000 para siempre. Si **sigue al máximo intradía**, el piso sube con tu mejor momento,
aunque no hayas cerrado nada:

*Ejemplo:* estás +1.500 sin realizar (máximo 51.500), el precio vuelve y cerrás en 50.000. El piso
subió a 49.500: ahora te quedan **1.500** antes de fallar, y nunca perdiste plata realizada. Con
«traba», el piso deja de subir cuando llega al saldo inicial.

El simulador **no simula** la caída que sigue al máximo intradía: hace falta el capital vela por
vela, no solo el cierre de cada operación. No la aproxima: la deja afuera y avisa que la
probabilidad de pasar es un **tope superior** (la real es más baja).

**4. Lo que queda fuera de la app.** Pagar una evaluación real es dinero real: queda fuera de la
app (CLAUDE.md §5). Para cargar las reglas de una firma real en el simulador hace falta su
reglamento publicado, con el enlace y la fecha de lectura; hoy no hay ninguna cargada.

> **Probá antes de seguir:** cuenta de 100.000, caída de 5.000 que sigue al máximo al cierre del
> día, sin traba. Cerrás el día 1 en 103.000 y el día 2 en 99.000. ¿Fallaste?
>
> <details><summary>Ver la respuesta</summary>
>
> **Todavía no, por poco.** Después del día 1 el piso subió a 103.000 − 5.000 = 98.000, y 99.000
> está arriba: te queda solo 1.000 de margen, aunque estés apenas 1.000 abajo del saldo inicial.
> Con caída fija, el piso seguiría en 95.000 y te quedarían 4.000.
>
> </details>

## Interactivo

El **simulador de cuenta fondeada** con la plantilla genérica: completá las reglas de un reglamento
y mirá qué errores marca el motor si falta un valor o no cierra.

## Ejercicio

Tres reglamentos **inventados** (de práctica, no de firmas reales). ¿Cuál es el más peligroso y por
qué?

- **A:** objetivo 8 %, caída máxima 10 % fija, pérdida diaria 5 % del tamaño, sin límite de días.
- **B:** objetivo 6 %, caída máxima 4 % que sigue al máximo intradía, sin traba.
- **C:** objetivo 10 %, caída máxima 10 % fija, como mucho 30 días operados.

<details><summary>Ver la solución</summary>

**B.** El margen (4 %) es más chico que el objetivo (6 %) y además sube con cada ganancia sin
realizar. C tiene el problema del tiempo, pero un margen igual al objetivo; A es la más holgada.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada todavía: el simulador se abre en la lección 12.4.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Plantilla genérica en blanco | `Plantilla genérica (completá los valores)` | `config/propsim.toml` `templates.generica.name` | A |
| Una firma real necesita su reglamento | `necesita su reglamento publicado` | `docs/modules/propsim.md` | A |
| Pérdida diaria desde el tamaño o desde el saldo del día | `pérdida diaria desde el saldo al inicio del día (% del tamaño o de ese saldo)` | `docs/modules/propsim.md` | A |
| Tres modos de caída máxima | `TrailingIntraday` | `engine/crates/propsim/src/rules.rs` | A |
| Regla de consistencia | `Mejor día / ganancia total como mucho (fracción).` | `engine/crates/propsim/src/rules.rs` | A |
| La caída intradía no se simula y pasar queda como tope | `el resultado dice que pasar es un tope superior` | `docs/modules/propsim.md` | A |
| *Express Funded* de Topstep: mercados simulados | — | [Topstep, preguntas frecuentes](https://www.topstep.com/faq/), leída el 26-11-2026 | A |
| Pagar una evaluación real queda fuera de la app | `Pagar una evaluación real es dinero real` | `docs/design/2026-11-academia.md` | A |
