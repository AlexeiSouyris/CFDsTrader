# 9.4 Walk-forward y fuera de muestra

> **Nivel 9 · Backtesting y sus trampas.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Un
> [backtest](../GLOSARIO.md#backtest) es el pasado simulado: no dice nada seguro del futuro.

## Objetivo

Probar una regla **avanzando en el tiempo** ([walk-forward](../GLOSARIO.md#walk-forward): entrenar
24 meses, validar 3, avanzar 3) y guardar **12 meses que no se tocan** hasta el final
([fuera de muestra](../GLOSARIO.md#fuera-de-muestra)).

## La idea

Si una regla se ajusta y se evalúa con los mismos datos, la evaluación mide cuánto se pudo ajustar,
no cuánto vale. Hay que evaluarla con datos que **no** se usaron para elegirla.

**1. Walk-forward.** Los valores están en `config/data_split.toml`:

- Se **entrena** (o se fijan los parámetros) con **24 meses**.
- Se **valida** con los **3 meses siguientes**, que el ajuste no vio.
- La ventana **avanza 3 meses** y se repite.

El resultado es una serie de ventanas de validación pegadas, todas «fuera» de su propio
entrenamiento. Con validación de 02-2013 a 10-2025 (153 meses) entran **51 ventanas**. El informe
(`walk_forward_report.v1`) muestra cada ventana, cuántas dieron positivo, su estabilidad y la
comparación con comprar y mantener.

**2. Muchas ventanas, no un período lindo.** En US100, la estrategia en uso daba **+0,07 %** por
ventana con 8 ventanas (2023-11 a 2025-10, R006): parecía estar en cero. Con la historia completa,
51 ventanas desde 2013, da **−6,1 %** por ventana (R007, **peor**): las 8 ventanas eran un tramo
favorable. Una prueba con un solo período, o con pocas ventanas, puede decir cualquier cosa.

**3. El fuera de muestra: 12 meses que no se tocan.** Los últimos 12 meses completos de datos
quedan afuera de toda la investigación: hoy, desde el **01-11-2025**. No se cargan, no se miran y no
se usan para ajustar nada.

- El motor lo hace cumplir: la guarda `research_period` rechaza cualquier corrida que lo toque, y
  el Laboratorio no tiene un botón para usarlo.
- Solo una hipótesis **confirmatoria congelada que pasó** se evalúa ahí, **una sola vez** por hash
  (nivel 10). Si se mira dos veces, ya no es fuera de muestra: es otro período de ajuste.
- Los exámenes de replay de la Academia tampoco sortean fechas de esos meses.

**4. Otras pruebas de robustez que acompañan al walk-forward:**

- **Mitades:** la ventaja tiene que existir en las dos mitades del período (por ejemplo,
  2013–2020 y 2021–2025). Una ventaja que vive solo en una mitad es frágil.
- **Costos × 1,5:** todo el resultado se recalcula con los costos 1,5 veces más altos. Si la
  ventaja desaparece, dependía de suponer costos bajos (costos nunca en cero, nivel 2).
- **Contra comprar y mantener:** la referencia de los tres índices con los costos del CFD (R018).

> **Probá antes de seguir:** probás una regla en walk-forward, no te gusta el resultado, cambiás un
> parámetro y volvés a correr. Repetís 15 veces hasta que da bien. ¿El walk-forward te protegió?
>
> <details><summary>Ver la respuesta</summary>
>
> No. Cada vuelta usó el resultado de la validación para cambiar la regla: las ventanas de
> validación se convirtieron en datos de ajuste. Esas 15 vueltas son 15 pruebas (lección 9.3). Por
> eso la regla se congela **antes** de correr (pre-registro, nivel 10) y el fuera de muestra se mira
> una sola vez.
>
> </details>

## Interactivo

Un `walk_forward_report.v1` real en el detalle de una corrida: ventanas, porcentaje de ventanas
positivas, comprar y mantener y el tramo fuera de muestra.

## Ejercicio

Con 24 meses de entrenamiento, 3 de validación y paso de 3: si los datos empiezan en 01-2011, ¿en
qué mes empieza la primera validación y en cuál termina?

<details><summary>Ver la solución</summary>

Entrenamiento de 01-2011 a 12-2012 (24 meses); la primera validación va de **01-2013 a 03-2013**.
(Nuestras pruebas empiezan la validación en 02-2013 porque los futuros empiezan en 02-2011.)

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Walk-forward y fuera de muestra** en el detalle de una corrida: cada ventana, sus métricas y la
  comparación con comprar y mantener.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Meses de entrenamiento | 24 (`24`) | `config/data_split.toml` `walk_forward.train_months` | A |
| Meses de validación | 3 (`3`) | `config/data_split.toml` `walk_forward.validation_months` | A |
| Paso de la ventana | 3 (`3`) | `config/data_split.toml` `walk_forward.step_months` | A |
| Meses fuera de muestra | 12 (`12`) | `config/data_split.toml` `out_of_sample.months` | A |
| Fuera de muestra desde | `fuera de muestra intocable desde el 01-11-2025` | `docs/analysis/2026-11-autoanalisis.md` | A |
| US100 con 8 ventanas | `+0.07 %` | `docs/analysis/registro-de-resultados.csv` `R006` | A |
| US100 con 51 ventanas | `2013-02 a 2025-10 (51 ventanas)` | `docs/analysis/registro-de-resultados.csv` `R007` | A |
| De +0,07 % a −6,1 % | `de +0.07 % a -6.1 % por ventana` | `docs/analysis/registro-de-resultados.csv` `R007` | A |
| Una confirmatoria se evalúa una sola vez | `puede evaluarse en el fuera de muestra` | `docs/design/2026-11-laboratorio.md` | A |
| El replay no sortea fechas del fuera de muestra | `SOLO dentro del período de investigación` | `config/academy.toml` | A |
| Estrés de costos | × 1,5 (`1.5`) | `config/research.toml` `rule.cost_stress` | A |
| Regla: costos nunca en cero | `Costos nunca en cero:` | `CLAUDE.md` | A |
| Comprar y mantener con costos del CFD | `Comprar y mantener el CFD (con spread y financiación)` | `docs/analysis/registro-de-resultados.csv` `R018` | A |
