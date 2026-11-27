---
modulo: news
ruta: `intelligence/cfds_intel/news`
lenguaje: Python
estado: hecho
resumen: Ingesta de noticias en tiempo real, resumen e interpretación (positivo/negativo, impacto).
---

# news

## Responsabilidad (única)

- Ingesta de noticias en tiempo real, resumen e interpretación (positivo/negativo, impacto). Hoy: lector de los RSS oficiales de la Fed y del BCE (ADR 0006) a `news_item.v1`, con la hora de publicación real y `config/news.toml`
- guardado en `news_items` (solo agregar) con `python -m cfds_intel.news`
- reglas de la interpretación (`interpret.py`: intérprete reemplazable, rangos, instrumentos de la config, sin interpretar antes de la publicación) que emite `news_interpretation.v1`. intérprete con un modelo local en Ollama, solo en localhost y con el modelo fijado por digest (`ollama.py`, `config/news_interpreter.toml`, ADR 0008), que lee el texto oficial de cada comunicado sin guardarlo (`fulltext.py`, que sigue el enlace al documento en las actas del FOMC y las proyecciones, HTML o PDF, #168; cuando no entra entero, quita menús del sitio y elige el inicio y las secciones clave configuradas, #190), y la prueba de selección del modelo (`select_model.py`)
- guardado de las interpretaciones en `news_interpretations` (solo agregar) y publicación en el stream `news_interpretations.v1` (`interpretations.py`), con el comando `python -m cfds_intel.news.interpret_new` (#186). El ajuste de −2 a +2 del puntaje lo calcula `strategy` (Rust)

## Estado

- lector de RSS (#109), guardado (#111), contrato y reglas de la interpretación (#122), intérprete Ollama y prueba de selección (#129), documento enlazado (#168), guardado y publicación de interpretaciones (#186), secciones clave del texto oficial (#190)
