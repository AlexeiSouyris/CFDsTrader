# Novedad de los comunicados del FOMC, 2011–2025 (descriptivo)

- **Issue:** #668 (plan del autoanálisis, #658, ítem 8, **N1**). **Fecha:** 2026-11-25.
- **Qué es:** cuánto cambió el texto de cada comunicado programado del FOMC respecto del anterior. **Solo describe la serie.** No se miran precios ni retornos, no se prueba ninguna hipótesis y no se gastó presupuesto de pruebas. No dice nada sobre si se puede ganar con esto.
- **Serie:** [`2026-11-fomc-novedad.csv`](2026-11-fomc-novedad.csv) (117 filas). La genera `python -m cfds_intel.fomc_novelty --dry-run --csv ...` (módulo [`fomc_novelty`](../modules/fomc_novelty.md)).
- **Fuente:** Board of Governors of the Federal Reserve System, federalreserve.gov (dominio público; piden citarla). Los comunicados son los de `official_releases` (ADR 0016); el texto, el de la página de cada uno.

## Cómo se mide

- **Texto:** los párrafos del cuerpo de la página oficial hasta el párrafo de la votación ("Voting for…"), que se deja fuera porque cambia con la rotación de miembros, no con la política.
- **`word_diff` (versión `word_diff.v1`):** palabras en minúscula y `difflib` sin *autojunk*. Valor = 1 − 2·M/(n₁ + n₂), con M las palabras de los bloques iguales y en el mismo orden. 0 = mismo texto; 1 = nada en común.
- **`embedding_cosine` (versión `embedding_cosine.v1`):** distancia coseno entre los embeddings de los dos textos, con Ollama local y `nomic-embed-text` (licencia Apache 2.0 según su ficha; el dueño la confirma al instalarlo), prefijo `clustering: `, sin recortar el texto. Cada fila guarda el nombre y el **digest** del modelo. **No se calculó todavía:** el modelo no está instalado en el Mac mini (ver Pendientes).
- **Solo comunicados programados** (118, de 2011-01-26 a 2025-12-10; la primera no tiene con qué compararse). Los 4 no programados quedan fuera por configuración: el del 11-10-2019 es una nota técnica de implementación sin votación, y los de marzo de 2020 son de emergencia. Por eso el comunicado del 29-4-2020 se compara con el del 29-1-2020.
- **Desde cuándo vale cada valor:** desde la hora del comunicado (`available_at` = `scheduled_at` de `official_releases`). Usa ese texto y el anterior, nada posterior. Antes de 2020 esa hora sale de la regla del ADR 0016, con ±15 minutos.

## Sobre mirar el futuro

- El diff de palabras es aritmética sobre dos textos públicos: no mira el futuro.
- Un modelo de embeddings se entrenó con textos hasta su fecha de corte, que puede incluir comentarios posteriores sobre estos comunicados. Acá eso pesa poco, porque no se le pide ninguna opinión ni predicción: solo un vector del texto, y la medida es la distancia entre dos vectores del mismo modelo. Aun así queda registrado el digest del modelo, y un estudio que use esta serie lo tiene que decir.

## Resultados (`word_diff`, 117 comunicados)

| Estadística | Valor |
|---|---|
| Mínimo | 0,012 (14-12-2022) |
| Percentil 25 | 0,059 |
| Mediana | 0,104 |
| Percentil 75 | 0,173 |
| Máximo | 0,635 (29-4-2020) |
| Media | 0,137 |
| Largo del texto (palabras) | de 248 a 800, mediana 415 |

**Mediana por año:**

| Año | n | Mediana | Mín. | Máx. |
|---|---|---|---|---|
| 2011 | 6 | 0,266 | 0,104 | 0,518 |
| 2012 | 8 | 0,194 | 0,055 | 0,532 |
| 2013 | 8 | 0,081 | 0,034 | 0,208 |
| 2014 | 8 | 0,065 | 0,032 | 0,367 |
| 2015 | 8 | 0,109 | 0,076 | 0,423 |
| 2016 | 8 | 0,102 | 0,043 | 0,173 |
| 2017 | 8 | 0,116 | 0,062 | 0,251 |
| 2018 | 8 | 0,074 | 0,055 | 0,242 |
| 2019 | 8 | 0,124 | 0,085 | 0,208 |
| 2020 | 7 | 0,066 | 0,013 | 0,635 |
| 2021 | 8 | 0,056 | 0,049 | 0,219 |
| 2022 | 8 | 0,134 | 0,012 | 0,478 |
| 2023 | 8 | 0,054 | 0,021 | 0,150 |
| 2024 | 8 | 0,092 | 0,014 | 0,317 |
| 2025 | 8 | 0,117 | 0,052 | 0,133 |

**Los 10 comunicados más nuevos:**

| Comunicado | Contra | Valor | Palabras (antes → ahora) |
|---|---|---|---|
| 2020-04-29 | 2020-01-29 | 0,635 | 273 → 384 |
| 2012-09-13 | 2012-08-01 | 0,532 | 380 → 470 |
| 2011-06-22 | 2011-04-27 | 0,518 | 420 → 414 |
| 2022-03-16 | 2022-01-26 | 0,478 | 385 → 274 |
| 2020-09-16 | 2020-07-29 | 0,441 | 408 → 418 |
| 2015-12-16 | 2015-10-28 | 0,423 | 515 → 556 |
| 2011-11-02 | 2011-08-09 | 0,404 | 450 → 433 |
| 2014-10-29 | 2014-09-17 | 0,367 | 748 → 601 |
| 2011-08-09 | 2011-06-22 | 0,336 | 414 → 450 |
| 2024-01-31 | 2023-12-13 | 0,317 | 310 → 287 |

Lo que se ve, sin interpretarlo como señal: los valores altos caen en reuniones conocidas por cambiar el mensaje (el primer comunicado después de la emergencia de 2020, el QE3 de septiembre de 2012, la primera suba de diciembre de 2015 y la de marzo de 2022, el fin de las compras en octubre de 2014, el nuevo marco de septiembre de 2020). La mayoría de los comunicados repite casi todo el anterior (mediana ≈ 10 % de palabras cambiadas), y 2011–2012 cambiaban más que los años siguientes.

## Límites

- **El diff de palabras mide forma, no significado:** reordenar párrafos o cambiar un número cuenta igual que cambiar una idea. Por eso está el embedding.
- **118 comunicados en 15 años:** una muestra chica. Cualquier prueba con precios tiene que pre-registrarse y contar como prueba nueva en el registro de resultados.
- **El formato de la página puede cambiar:** si un comunicado no tiene el cuerpo o la votación donde se esperan, la carga falla (no adivina). Así pasó con el del 11-10-2019, que por eso quedó fuera.

## Pendientes (para el dueño)

1. **Instalar el modelo de embeddings** (`ollama pull nomic-embed-text`, unos 270 MB según la biblioteca de Ollama; es una descarga, así que no la hizo el agente) y correr `--method embedding_cosine`. Las páginas ya quedaron en `data/raw/fomc_statements` y no se vuelven a pedir.
2. **Guardar la serie en la base** (después del merge, que aplica la migración 0050): `DATABASE_URL=... uv run python -m cfds_intel.fomc_novelty` en el Mac mini.
3. Si se quiere usar en una estrategia: un Issue aparte con una hipótesis pre-registrada.
