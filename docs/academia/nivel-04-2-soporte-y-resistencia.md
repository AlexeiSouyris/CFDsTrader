# 4.2 Soporte y resistencia objetivos

> **Nivel 4 · Estructura.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los ejemplos son
> redondos, no cotizaciones.

## Objetivo

Armar un [soporte](../GLOSARIO.md#soporte) o una [resistencia](../GLOSARIO.md#resistencia) como los
define el motor: un precio **tocado al menos 3 veces**, con una **tolerancia** medible, y no "una
zona que se ve".

## La idea

"Acá hay un soporte" es una opinión hasta que se dice cuántos toques, en qué ventana y con qué
margen. El motor lo fija así (`config/patterns.toml`, spec §5.3):

- **Soporte** = un grupo de **al menos 3 pivotes bajos confirmados** de las **últimas 200 velas**,
  todos dentro de **±0,15 %** del promedio del grupo. El precio del nivel es **ese promedio**.
- **Resistencia** = lo mismo con pivotes altos.
- Solo cuentan pivotes **ya confirmados** (lección 4.1): un nivel nunca se arma con una suposición.

**Cómo arma los grupos.** Ordena los precios de los pivotes de menor a mayor y los va sumando al
grupo mientras todos sigan a ±0,15 % del promedio; si el siguiente no entra, cierra el grupo y
empieza otro. Un grupo con menos de 3 pivotes no es nivel.

**El rechazo** (el gatillo del motor, para largos): la vela toca el soporte (su mínimo llega a
nivel × (1 + 0,15 %) o más abajo) y **cierra arriba** del nivel. Para la resistencia es el espejo.

**Por qué objetivo.** Con una regla fija se puede contar cuántas veces un nivel "funcionó" y cuántas
no, y comparar contra el azar. Con una línea a ojo, siempre se encuentra un nivel que funcionó...
después.

> **Probá antes de seguir:** pivotes bajos en 5.990, 5.994 y 5.998. ¿Forman un soporte?
>
> <details><summary>Ver la respuesta</summary>
>
> Sí. El promedio es 5.994 y el 0,15 % de 5.994 son unos 9 puntos: los tres están a 4 puntos o
> menos. Soporte en **5.994**, con 3 toques.
>
> </details>

## Interactivo

Dibujás tu línea en un día real; después se prende la capa de soportes y resistencias del motor y
ves la diferencia: cuáles de tus líneas coinciden con un nivel del motor y qué niveles no viste.

## Ejemplo resuelto

Al mismo soporte de 5.994 se le suma un cuarto pivote bajo en 6.010.

- Promedio de los cuatro: (5.990 + 5.994 + 5.998 + 6.010) ÷ 4 = 5.998. El 0,15 % de 5.998 son
  unos 9 puntos.
- 6.010 queda a 12 puntos del promedio: **no entra**. El grupo de 5.990–5.998 se cierra (soporte en
  5.994) y 6.010 empieza otro grupo, con 1 solo pivote: no es nivel.
- **Rechazo:** una vela con mínimo 5.999 y cierre 6.004 toca el soporte (5.999 ≤ 5.994 × 1,0015 ≈
  6.003) y cierra arriba de 5.994: es un rechazo.

## Ejercicio

Elegí un día real en el replay de práctica y marcá 3 niveles. Para cada uno, anotá los pivotes que
lo forman y comprobá la regla: ¿al menos 3?, ¿todos a ±0,15 % del promedio?, ¿todos de las últimas
200 velas?

<details><summary>Ver la solución</summary>

La solución es la capa del motor: si tu nivel no aparece, revisá qué condición falló (casi
siempre: solo 2 toques, o un toque fuera de la tolerancia). Si el motor tiene un nivel que no
marcaste, buscá sus 3 pivotes.

</details>

## Examen

**Examen de replay de niveles.** El motor elige un gráfico real con la fecha oculta (300 velas de
calentamiento y 400 de examen) y vos marcás tus niveles. Al terminar, el motor calcula **sus**
soportes y resistencias con las velas que viste (hasta el cursor final) y compara:

- una marca **acierta** si queda a **0,25 ATR** o menos de un nivel del motor (con el
  [ATR](../GLOSARIO.md#atr) del cursor; por ejemplo, con un ATR de 12 puntos, a 3 puntos);
- **precisión** = marcas que aciertan ÷ marcas; **cobertura** = niveles del motor que marcaste ÷
  niveles del motor.

Aprobás con una precisión de al menos el 80 %; la cobertura se muestra para aprender, pero no
decide. Los umbrales son **provisorios** hasta que el dueño los apruebe.

## Desbloquea

- **Dibujo**: líneas horizontales, de tendencia y rectángulos sobre el gráfico.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Toques mínimos de un nivel | 3 (`3`) | `config/patterns.toml` `levels.min_touches` | A |
| Ventana de los pivotes | 200 velas (`200`) | `config/patterns.toml` `levels.lookback_bars` | A |
| Tolerancia alrededor del promedio y del rechazo | ±0,15 % (`0.15`) | `config/patterns.toml` `levels.tolerance_pct` | A |
| Rechazo: toca y cierra del lado correcto | `L[t] ≤ nivel × (1 + 0,15 %) y C[t] > nivel` | `docs/spec/fase-1-especificacion.md` §5.3 | A |
| El nivel es el promedio del grupo; grupos por precio ordenado | `nivel = ese promedio` | `engine/crates/patterns/src/levels.rs` | A |
| Tolerancia del examen de niveles (provisoria) | 0,25 ATR (`0.25`) | `config/academy.toml` `replay.level_tolerance_atr` | A |
| Precisión para aprobar (provisorio) | 80 % (`80`) | `config/academy.toml` `thresholds.quiz_pass_pct` | A |
| Velas de calentamiento y de examen | 300 (`300`) | `config/academy.toml` `replay.warmup_bars` | A |
| Velas del examen | 400 (`400`) | `config/academy.toml` `replay.exam_bars` | A |
| El examen compara con los niveles del motor hasta el cursor final | `con las velas hasta el cursor final` | `engine/crates/academy/src/grade.rs` | A |
