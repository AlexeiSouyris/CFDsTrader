# 0.2 Cuántos pierden

> **Nivel 0 · Orientación.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)).

## Objetivo

Saber el dato duro antes de empezar: cuántas cuentas minoristas de [CFD](../GLOSARIO.md#cfd)
pierden dinero, de dónde sale ese número y qué dice (y qué no dice) sobre vos.

## La idea

En 2018 el regulador europeo de valores ([ESMA](../GLOSARIO.md#esma)) juntó los estudios de varios
reguladores nacionales sobre clientes minoristas de CFD. En todos, **la mayoría perdió dinero**:

| Regulador | Período estudiado | Clientes que perdieron |
|---|---|---|
| Francia (AMF) | 2009 a 2013 | más del 89 % |
| España (CNMV) | 21 meses, de 2015 a 2016 | aproximadamente el 82 % |
| Noruega (Finanstilsynet) | 1 a 2 años, unos 1.000 clientes | 82 % |
| Polonia (KNF) | 2016 | 79,28 % |
| Chipre (CySEC) | enero a agosto de 2017 | 76 % en promedio |
| Irlanda (CBI) | 2013 y 2014 | 75 % (74 % en una revisión posterior) |

Por eso la advertencia estándar que exige la regla dice que **entre el 74 % y el 89 %** de las
cuentas minoristas pierden dinero con CFD. La medida de ESMA era temporal; en España la CNMV adoptó
las mismas reglas en 2019, sin fecha de fin (siguen mientras la CNMV no las derogue), y las
complementó en 2023.

La regla obliga además a **cada proveedor** a publicar el porcentaje de **sus propias** cuentas
minoristas que pierden dinero. Ese número cambia con el tiempo y lo ves en la web de cada broker.

**No pasa solo con los CFD.** Barber y Odean (2000) estudiaron 66.465 hogares con cuenta en un
broker de EE. UU. entre 1991 y 1996. Los que **más operaban** ganaron un 11,4 % al año, mientras
el mercado dio un 17,9 %. Operar mucho tuvo un costo.

**Qué no dice este dato.** No dice que vos vas a perder, ni que vas a estar en el grupo que gana.
Dice cuántos perdieron en el pasado: es la **tasa base**, el punto de partida contra el que hay que
medir cualquier idea. Si un método no puede mostrar con datos que hace algo distinto, lo más
probable es que termine como la mayoría.

> **Probá antes de seguir:** si 8 de cada 10 cuentas pierden, y alguien te muestra 3 operaciones
> ganadoras seguidas, ¿eso prueba que su método funciona?
>
> <details><summary>Ver la respuesta</summary>
>
> No. Con tan pocas operaciones el azar explica casi cualquier cosa. Lo vas a ver con números en el
> nivel 8 (rachas y suerte).
>
> </details>

## Interactivo

Una tarjeta con la cita de ESMA y su fuente, y la tabla de estudios nacionales de arriba. Cada
número enlaza a su documento original.

## Ejercicio

Buscá en la web de un broker de CFD europeo su advertencia de riesgo y anotá qué porcentaje de sus
cuentas minoristas pierde dinero. Compará ese número con el rango de ESMA.

<details><summary>Ver la solución</summary>

La advertencia tiene que dar el porcentaje de las cuentas minoristas de ese proveedor que pierden
dinero con CFD. El número exacto cambia con el tiempo y entre brokers; lo importante es encontrarlo
y compararlo con el 74 % a 89 % general.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Glosario interactivo**: las palabras subrayadas de toda la Terminal muestran su definición.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Cuentas minoristas de CFD que pierden (advertencia estándar) | 74 % a 89 % | [ESMA, aviso de las decisiones sobre CFD (ESMA35-43-1135, 22-05-2018), anexo II, secciones D y E](https://www.esma.europa.eu/sites/default/files/library/esma35-43-1135_notice_of_pi_decisions_on_cfds_and_binary_options.pdf), leído el 26-11-2026 | A |
| Estudios nacionales (AMF 89 %, CNMV 82 %, CySEC 76 %, CBI 74–75 %, KNF 79,28 %, Finanstilsynet 82 %) | — | [Decisión (UE) 2018/796 de ESMA, considerandos 35 y 36 (DO L 136 de 01-06-2018)](https://eur-lex.europa.eu/eli/dec/2018/796/oj), leída el 26-11-2026 | A |
| Advertencia con el porcentaje propio de cada proveedor | — | [ESMA, aviso ESMA35-43-1135, anexo II, secciones B y C](https://www.esma.europa.eu/sites/default/files/library/esma35-43-1135_notice_of_pi_decisions_on_cfds_and_binary_options.pdf); en España, anexo II de la resolución de la CNMV de abajo | A |
| Mismas medidas en España, sin fecha de fin (CFD desde el 01-08-2019; complementadas el 11-07-2023) | — | [Resolución de la CNMV de 27-06-2019 (BOE-A-2019-9737)](https://www.boe.es/buscar/act.php?id=BOE-A-2019-9737), leída el 26-11-2026 | A |
| Barber y Odean: 66.465 hogares, 1991–1996, 11,4 % frente a 17,9 % | — | [Barber y Odean (2000), *Trading Is Hazardous to Your Wealth*, Journal of Finance 55(2), 773–806](https://ideas.repec.org/a/bla/jfinan/v55y2000i2p773-806.html), resumen leído el 26-11-2026 | A |
| La medida de ESMA está en nuestro catálogo (cierre por margen) | 50 % (`50`) | `config/instruments.toml` `account.margin_closeout_pct` (fuente `esma-2018-796`) | A |
