# 9.2 Sesgo de supervivencia

> **Nivel 9 · Backtesting y sus trampas.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Un
> [backtest](../GLOSARIO.md#backtest) es el pasado simulado: no dice nada seguro del futuro.

## Objetivo

Reconocer cuándo una prueba mira **solo lo que sobrevivió** (acciones, fondos, traders o ideas) y
cómo se evita.

## La idea

Si para probar el pasado se usa la lista de **hoy**, todo lo que desapareció en el camino queda
afuera: las empresas que quebraron, los fondos que cerraron, los traders que dejaron. El pasado se
ve mejor de lo que fue.

**1. En acciones.** «Comprar desde 2010 las acciones que hoy están en el Nasdaq-100» elige con el
diario de hoy: esas empresas están en el índice **porque** les fue bien. Las que salieron no entran
en la prueba. La versión honesta usa, en cada fecha, **los componentes que había ese día**.

- El proyecto tiene esa lista: el módulo `universe` da los componentes del Nasdaq-100 en cada
  fecha, sin sesgo de supervivencia (ADR 0005), con un paquete que cubre desde el 1-1-2003.
- Faltan los **precios** de las empresas que dejaron de cotizar: ninguna fuente gratuita los da
  con condiciones que sirvan (ADR 0005, §2). Por eso los análogos v1 usan solo los índices. Antes
  de probar una estrategia de acciones sueltas, ese hueco se tiene que cerrar.

**2. En fondos.** Brown, Goetzmann, Ibbotson y Ross (1992) mostraron que una muestra de fondos
recortada por la supervivencia puede hacer **parecer predecible** el rendimiento aunque no lo sea:
los fondos que sobreviven son, en parte, los que tuvieron suerte.

**3. En traders y cursos.** Ves a los que ganaron un campeonato o pasaron una evaluación de una
cuenta fondeada, no a los que se quedaron en el camino. Sin el total de los que lo intentaron, cinco
casos de éxito no dicen nada (nivel 12).

**4. En ideas, incluidas las nuestras.** Si solo se publicaran las pruebas que salieron bien,
veríamos las ideas sobrevivientes, no cuántas se probaron. Por eso el registro de resultados guarda
**toda** prueba, en el mismo PR que su informe, aunque no pase, y las filas viejas no se borran.
Hoy, de las hipótesis que votaron, **ninguna** pasó: eso también es un resultado.

**5. Por qué los índices nos afectan menos.** US500, US100 y US30 siguen a su índice, que cambia
sus componentes con reglas propias. Probar una regla **sobre el índice** no tiene este sesgo: el
precio del índice de cada día ya incluía a las empresas que después salieron. El sesgo aparece si
se eligen **acciones** del índice con la lista de hoy.

> **Probá antes de seguir:** un informe dice «las 5 acciones con más ganancia de los últimos 10
> años subieron un 2.000 %; una estrategia que las hubiera comprado habría ganado eso». ¿Qué está
> mal?
>
> <details><summary>Ver la respuesta</summary>
>
> Elige las acciones **después** de saber cuáles subieron: es sesgo de supervivencia y mirar el
> futuro a la vez. Hace 10 años nadie sabía cuáles iban a ser las 5 mejores. La prueba honesta
> elige con lo que se sabía en cada fecha.
>
> </details>

## Interactivo

Sin interactivo propio: la lección se apoya en el registro de resultados, que se abre en la 9.5.

## Ejercicio

Para cada caso, decí si tiene sesgo de supervivencia:

1. Un backtest de una regla de tendencia sobre el futuro del S&P 500 desde 2011.
2. Un ranking de los fondos que existen hoy, ordenados por su rendimiento de los últimos 15 años.
3. Un curso que muestra los extractos de sus 10 mejores alumnos.

<details><summary>Ver la solución</summary>

1. No, por el lado de los componentes: el índice ya los cambia solo. (Igual hay que cuidar los
   costos, el roll y el fuera de muestra.)
2. Sí: faltan los fondos que cerraron, que suelen ser los que rindieron peor.
3. Sí: faltan los alumnos que perdieron, y no se sabe cuántos eran.

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada nuevo.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Componentes del Nasdaq-100 en cada fecha | `Componentes del NASDAQ 100 en cada fecha, sin sesgo de supervivencia` | `docs/modules/universe.md` | A |
| Desde cuándo cubre la lista | `desde el 1-1-2003` | `docs/decisions/0005-datos-para-analogos.md` | A |
| Precios de empresas que dejaron de cotizar: ninguna fuente gratis sirve | `ninguna de las cuatro candidatas cumple` | `docs/decisions/0005-datos-para-analogos.md` | A |
| Los análogos v1 usan solo los índices | `los análogos v1 usan solo los índices` | `docs/decisions/0005-datos-para-analogos.md` | A |
| Toda prueba entra al registro, aunque no pase | `en el mismo PR que su informe, aunque no pase` | `docs/analysis/registro-de-resultados.md` | A |
| Las filas viejas no se borran | `Las filas viejas no se editan ni se borran.` | `docs/analysis/registro-de-resultados.md` | A |
| Ninguna hipótesis pasó | `ninguna pasa` | `docs/analysis/2026-11-regla-presupuesto-global.md` | A |
| Ejemplo del registro: H5.1 no pasa | `no pasa` | `docs/analysis/registro-de-resultados.csv` `R035` | A |
| Muestras recortadas por supervivencia hacen parecer predecible el rendimiento | — | [Brown, Goetzmann, Ibbotson y Ross (1992), *Survivorship Bias in Performance Studies*, Review of Financial Studies 5(4), 553–580](https://ideas.repec.org/a/oup/rfinst/v5y1992i4p553-80.html), resumen leído el 26-11-2026 | A |
