# 11.2 Perfil de volumen

> **Nivel 11 · Avanzado, con advertencias.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los números
> de los ejemplos son redondos, de práctica: no son cotizaciones ni resultados.

## Objetivo

Leer un perfil de volumen (**POC** y **área de valor**), saber cómo lo arma el motor desde barras de
1 minuto y qué tiene de aproximado, y qué dio la única regla con perfil que probamos (H4.3).

## La idea

**1. Qué es.** El perfil de volumen de una sesión dice **cuánto se operó a cada precio**. Se dibuja
de costado: una barra horizontal por precio.

- **POC** (*point of control*): el precio con más [volumen](../GLOSARIO.md#volumen).
- **Área de valor**: el rango de precios alrededor del POC donde se operó el **70 %** del volumen.
  Su borde de arriba es el **VAH** y el de abajo, el **VAL**.

**2. Cómo lo arma el motor** (solo dentro de la prueba H4.3, `research::program4_c`):

- **Datos:** las barras de 1 minuto del futuro del Nasdaq (NQ) en el CME. El CFD de IG no tiene
  volumen, así que el perfil existe solo para los futuros.
- **Sesión:** de las 18:00 hora de Nueva York del día hábil anterior a las 17:00 del día. Si le
  falta más del 10 % de sus barras, ese perfil **no existe** (no se inventa).
- **Casillero** (el ancho de cada barra horizontal): tick × máx(1, redondeo(0,0002 × apertura ÷
  tick)), con tick 0,25. *Ejemplo:* si la sesión abre en 20.000, 0,0002 × 20.000 = 4 puntos = 16
  ticks: casilleros de **4 puntos**.
- **El reparto es una aproximación:** de una barra de 1 minuto se sabe el volumen total, el máximo y
  el mínimo, pero **no a qué precio** se operó cada contrato. El motor reparte el volumen en partes
  iguales a lo largo del rango de la barra. Un perfil hecho con cada operación sería distinto.
- **Área de valor:** se arranca en el POC y se suma, de a un casillero, el vecino (de arriba o de
  abajo) con más volumen, hasta llegar al 70 %.

*Ejemplo de reparto:* una barra con mínimo 20.000, máximo 20.008 y 800 contratos, con casilleros
de 4 puntos, deja 400 en el casillero 20.000–20.004 y 400 en el 20.004–20.008.

**3. Lo que dice la evidencia.** No conocemos estudios revisados que muestren que el POC o el área
de valor den ventaja por sí solos (las fuentes del temario para este nivel no traen ninguno). Lo
que sí tenemos es **nuestra prueba** de una regla concreta con perfil:

- **H4.3** (la «estrategia C» de Bruni, R033): perfil del día del CME desplazado, vuelta al área de
  valor de ayer y cierre sobre el VAH. Reglas congeladas antes de correr, US100, 2013–2025.
- En casi 13 años dio **128 señales** y **85 operaciones**, menos que el mínimo de 100.
- Pierde **antes** de costos (−0,17 R por operación) y después (−0,31 R), con 38,8 % de aciertos;
  el alfa contra comprar y mantener fue **−0,162 % por mes**. **No pasa** ningún criterio.
- Contra entradas al azar con los mismos stops, quedó **peor** que el 81 % de las simulaciones.

Una regla que no pasa no prueba que el perfil «no sirva para nada»: prueba que **esa** regla no tuvo
ventaja. Lo que falta es evidencia a favor.

> **Probá antes de seguir:** en una sesión, el casillero con más volumen es 20.040–20.044. ¿Qué
> es ese precio y qué te dice sobre mañana?
>
> <details><summary>Ver la respuesta</summary>
>
> Es el POC de la sesión: donde más se operó (según el reparto aproximado de las barras de 1
> minuto). Sobre mañana no dice nada probado: es una descripción de lo que pasó.
>
> </details>

## Interactivo

Hoy la app **no dibuja** el perfil de volumen: solo existe dentro de la prueba H4.3. Se puede leer
su informe (`docs/analysis/2026-11-programa-4-h43-resultados.md`), con cuántas sesiones tuvieron
perfil y cuántas velas quedaron después de cada condición de la regla.

## Ejercicio

Un perfil tiene 1.000 contratos en total. Casilleros (de abajo hacia arriba) con 50, 100, 200,
300 (POC), 150, 120 y 80. Armá el área de valor del 70 % con la regla del motor.

<details><summary>Ver la solución</summary>

Se arranca en el POC: 300. Vecinos: 200 (abajo) y 150 (arriba) → se suma 200 (500). Vecinos: 100
y 150 → 150 (650). Vecinos: 100 y 120 → 120 (770 ≥ 700). Área de valor: del casillero de 200 al
de 120, con el **77 %** del volumen.

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada. El temario prevé un «perfil de volumen (descriptivo)», pero la app todavía no lo dibuja:
queda **pendiente**, y cuando exista se suma su candado.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Área de valor del 70 % | `0.7` | `config/research.toml` `program4_c.value_area` | A |
| Sesión del perfil: desde las 18:00 | `18:00` | `config/research.toml` `program4_c.profile_session_start` | A |
| Sesión del perfil: hasta las 17:00 | `17:00` | `config/research.toml` `program4_c.profile_session_end` | A |
| Hora de Nueva York | `America/New_York` | `config/research.toml` `program4_c.timezone` | A |
| Barras mínimas de la sesión (90 %) | `0.9` | `config/research.toml` `program4_c.profile_min_bars_fraction` | A |
| Casillero: fracción de la apertura | `0.0002` | `config/research.toml` `program4_c.profile_bin_fraction` | A |
| Tick del NQ | `0.25` | `config/research.toml` `program4_c.tick` | A |
| Barras de 1 minuto del NQ | `con las barras de 1 minuto del NQ` | `config/research.toml` | A |
| Volumen repartido a lo largo de la barra | `Reparte el volumen de una barra en partes iguales a lo largo de [mínimo, máximo].` | `engine/crates/backtest/src/research/program4_c.rs` | A |
| H4.3: alfa por mes y operaciones | `-0.162 % (p 0.994; DSR 0.000; -0.226 % / -0.057 %; 85 < 100)` | `docs/analysis/registro-de-resultados.csv` `R033` | A |
| H4.3: señales | `128 señales` | `docs/analysis/2026-11-programa-4-h43-resultados.md` | A |
| H4.3: R por operación | `−0,31 veces el riesgo por operación después de costos (−0,17 antes de costos), 39 % de aciertos` | `docs/analysis/2026-11-programa-4-h43-resultados.md` | A |
| H4.3: aciertos | `38,8 %` | `docs/analysis/2026-11-programa-4-h43-resultados.md` | A |
| H4.3 contra el azar | `que el 81 % de las simulaciones` | `docs/analysis/2026-11-programa-4-h43-resultados.md` | A |
