# 0008 – Modelo abierto local para interpretar noticias

- **Estado:** propuesta
- **Fecha:** 2026-11-22
- **Aprobada por:** pendiente (el dueño; ver `CLAUDE.md`)
- **Issue:** #129 (relacionado: ADR 0006 §4, #122, #130, spec §9)

## Contexto

El ADR 0006 §4 decidió que la interpretación de noticias (resumen en español, sentimiento y relevancia por instrumento, spec §9) se hace con un **modelo abierto corriendo localmente**, sin pagar ninguna API. Dejó para otro ADR qué modelo, con qué licencia y cómo se evalúa. Ya está todo lo que lo rodea:
- el contrato `news_interpretation.v1` y las reglas de cualquier intérprete (#122);
- el ajuste del puntaje en Rust (#130).

Falta el intérprete real.

**Dónde corre:** en el Mac mini, que es donde se conservan los datos (`docs/guides/trabajo-en-paralelo.md`). Datos medidos el 22-11-2026, en el #129:

| | |
|---|---|
| Procesador | Apple M4 |
| Memoria | **16 GB**, compartidos con macOS, Docker (PostgreSQL y Redis) y el modelo |
| Disco libre | ~81 GB |
| Sistema | macOS 27.2 |
| Software de modelos | ninguno instalado (ni Ollama ni llama.cpp) |

**Candidatos.** Los datos salen de las fichas oficiales, leídas por el Mac mini el 22-11-2026 (#129). Lo que la ficha no dice figura como "no figura".

| Modelo | Licencia de los pesos | Política de uso aparte | Tamaño | Español | Memoria / archivo | Ejecución local |
|---|---|---|---|---|---|---|
| [Qwen3-8B](https://huggingface.co/Qwen/Qwen3-8B) | Apache-2.0 | no figura | 8,2B | "100+ languages and dialects" (no nombra el español) | GGUF Q4_K_M de 5,03 GB | GGUF oficial ([Qwen/Qwen3-8B-GGUF](https://huggingface.co/Qwen/Qwen3-8B-GGUF)) |
| [Ministral 3 8B Instruct 2512](https://huggingface.co/mistralai/Ministral-3-8B-Instruct-2512) | Apache-2.0 | no figura | 8,4B + 0,4B de visión | **sí, lo nombra** | "12GB of VRAM in FP8, and less if further quantized" | no figura en la ficha |
| [Gemma 4](https://gemma.ai/models/gemma-4/model-card/) (E4B o 12B) | [Apache 2.0](https://gemma.ai/resources/apache-2/) (Gemma 3 y anteriores tenían otros términos) | no, para Gemma 4 | E2B, E4B, 12B, 26B A4B y 31B | "35+ languages" (no nombra el español) | no figura | no figura en la ficha |
| [gpt-oss-20b](https://huggingface.co/openai/gpt-oss-20b) | Apache-2.0 | no figura en la ficha | 21B (3,6B activos) | no figura | "run within 16GB of memory" | Ollama oficial; solo con el formato "harmony" |

Software para correrlos: [Ollama](https://github.com/ollama/ollama/blob/main/LICENSE) y [llama.cpp](https://github.com/ggml-org/llama.cpp/blob/master/LICENSE), los dos con licencia **MIT**.

## Decisión (propuesta)

1. **Dónde y cómo corre:** en el Mac mini, con **Ollama**, escuchando solo en `localhost`. El intérprete de Python (`cfds_intel.news`) lo llama por HTTP local. Ninguna noticia ni respuesta sale de la máquina.
2. **Preselección: Qwen3-8B (Q4_K_M) y Ministral 3 8B, cuantizado a ~4 bits. Gemma 4 E4B queda de reserva** si los dos primeros no entran en memoria junto con Docker.
   - **gpt-oss-20b queda fuera:** con 16 GB compartidos está al límite, y además exige un formato de mensajes propio.
   - **Qwen3-14B y Gemma 4 12B quedan fuera de v1:** no hay datos de memoria verificados.
3. **El modelo final se elige con la prueba del punto 4, con criterios fijados ahora, antes de mirar resultados.** Lo elige el dueño con esos números. El elegido queda **fijado** por nombre, cuantización y hash del archivo, y eso se registra en `interpreter.model` de cada evento.
4. **Prueba de selección** (en el Mac mini, una sola vez por candidato; no gasta cupo de IG):
   - **Muestra:** 60 titulares de `news_items` (Fed y BCE). El dueño los etiqueta **antes** de correr los modelos: para cada instrumento afectado, el signo del sentimiento (−, 0 o +) y si es relevante (sí o no). Las etiquetas se guardan en el repo (`intelligence/tests/fixtures/`, solo título, enlace y etiqueta, como en el ADR 0006). Esos titulares no se usan para nada más.
     - **Hacen falta al menos 20 pares (titular, instrumento) marcados como relevantes.** Muchos titulares son institucionales, sin impacto. Si no se llega a 20, se agregan titulares a la muestra y se etiquetan **antes** de correr ningún modelo.
   - **Configuración:** temperatura 0, semilla fija y el mismo prompt para todos, guardado en el repo con su SHA-256 (`interpreter.prompt_sha256`).
   - **Criterios mínimos** (un candidato que no los cumple queda fuera):
     - ≥ 99 % de respuestas que pasan la validación de `interpret()` (#122): JSON válido, rangos e instrumentos de la config;
     - 100 % de **resultados estructurados** idénticos al correr dos veces lo mismo: los mismos instrumentos y el mismo ajuste por noticia que usa el puntaje, `redondear(2 × sentimiento × relevancia)` (#130). El texto del resumen puede variar: con aceleración Metal, Ollama no siempre es reproducible bit a bit, ni con temperatura 0 y semilla fija;
     - pico de memoria del proceso ≤ 8 GB, para dejar lugar a macOS y Docker;
     - ≤ 60 s por noticia (p95).
   - **Entre los que pasan:** gana el de mayor acierto de signo contra las etiquetas del dueño, solo en los pares que el dueño marcó como relevantes. Cada acierto se reporta con su **intervalo de confianza del 95 %** (Wilson). Si los intervalos de dos candidatos se superponen, o la diferencia es menor a 5 puntos, se toman como empatados y gana el más chico en memoria. El resultado de todos los candidatos se registra, incluidos los que fallaron (ADR 0003 §D).
   - **Si ningún candidato pasa los criterios mínimos,** no se elige ninguno y la decisión vuelve al dueño (otro candidato u otro criterio). Los umbrales no se relajan después de ver los resultados.
5. **Look-ahead en el backtest:** un modelo entrenado después de una noticia puede "saber" qué pasó después. Reglas:
   - En vivo, la interpretación vale desde `interpreted_at` (ya está así en el contrato).
   - **En el backtest solo se usan interpretaciones de noticias publicadas después de la fecha de corte de entrenamiento del modelo**, si el fabricante la publica. Si no la publica, se toma la fecha de publicación del modelo.
   - Las noticias anteriores se pueden interpretar para mostrarlas, pero no entran al backtest ni a la validación de la fase 5.
6. **Español:** el resumen va en español (spec §9). Si el modelo elegido no nombra el español en su ficha, la prueba del punto 4 lo controla: un resumen que no está en español cuenta como respuesta inválida.

## Alternativas consideradas

- **Una API paga** (por ejemplo un LLM en la nube): la descarta el ADR 0006 (enfoque FI de v1, sin costo).
- **Reglas fijas por palabras clave:** es gratis y determinista, pero no da relevancia por instrumento ni un resumen. Queda como línea base en la prueba: un modelo que no le gana no se usa.
- **gpt-oss-20b:** su licencia es buena, pero no entra con margen en 16 GB compartidos.
- **Correr el modelo en la nube de Claude Code:** la base de la nube se borra y el cupo de datos está en el Mac mini. Además, las noticias quedan mejor dentro de la máquina.

## Consecuencias

- **Hay que instalar Ollama** (MIT) en el Mac mini y bajar los dos o tres candidatos: unos 5 GB cada uno de los 81 GB libres. Lo hace el Mac mini con el ok del dueño (Issue con la etiqueta `mac-mini`).
- **El dueño etiqueta 60 titulares** antes de la prueba (PR #161), y algunos más si no llegan a 20 pares relevantes. Es el único trabajo manual. Los titulares son de noviembre de 2026, posteriores a la fecha de corte de todos los candidatos, así que la prueba de selección tampoco tiene fuga.
- **Más fácil:** cada evento dice qué modelo y qué prompt lo produjo, así que se puede reproducir y medir.
- **Más difícil:** el backtest solo tiene noticias interpretables desde la fecha de corte del modelo. Al principio va a haber poca historia de noticias para validar el ajuste (spec §9: "experimental" hasta la fase 5).
- **Después de aprobar este ADR** (en el #129): el `Interpreter` real con Ollama, la tabla de interpretaciones y el stream de Redis, y la prueba de selección como script reproducible.
- **Pendiente de verificar:** la cuantización GGUF de Ministral 3 8B, las fechas de corte de cada candidato y los requisitos de memoria de Qwen y Gemma. La prueba del punto 4 mide la memoria real.
