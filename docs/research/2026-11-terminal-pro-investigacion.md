# Investigación para la terminal profesional (épica #707)

Issue: #711. Fecha: 26-11-2026. Residencia del dueño: **España**. Todo corre en el **Mac mini**, sin dinero real.

Esto es **investigación, no una decisión**. Cada proveedor nuevo de datos, cada librería principal y cada cambio de spec necesita su ADR o su aprobación (`CLAUDE.md` §5c y §6). Las cuentas y las claves las crea el dueño: los agentes no crean cuentas.

Las páginas se leyeron el 26-11-2026. Lo que no se pudo leer queda marcado como **sin verificar**. No se supone nada.

---

## Resumen simple

1. **Lo que más vale y menos cuesta ya lo tenemos:** los datos de IG y Databento que ya están guardados alcanzan para un **modo "replay" vela a vela** y para **exámenes prácticos con el pasado**. Es la función estrella de TradingView y FX Replay, y no necesita proveedores nuevos.
2. **Noticias del mundo, gratis y legal:** **GDELT** permite guardar, redistribuir y usar sus datos sin pagar, con cita. Es la mejor base para "qué pasa en el mundo ahora".
3. **Petróleo y energía:** la **EIA** de EE. UU. es gratis y de dominio público. Tiene inventarios semanales de crudo, producción y precios.
4. **Buques en vivo:** **aisstream.io** da posiciones AIS en tiempo real gratis, con clave (cuenta con GitHub). **No publica términos de uso**: hay que preguntarles por escrito antes de guardar datos. AISHub exige tener un receptor propio. Global Fishing Watch es solo para uso no comercial y se centra en la pesca. MarineTraffic no tiene un plan gratis de API verificable.
5. **Cuentas fondeadas, con números reales:** Topstep publica que en 2025 se aprobó el **16,8 %** de las evaluaciones y que el **33,3 %** de quienes llegaron a la cuenta fondeada cobró algún pago. En la UE, entre el **74 % y el 89 %** de las cuentas minoristas de CFD pierden dinero (ESMA). Un simulador de reglas propio, honesto con estos números, es un buen módulo educativo.
6. **Proyectos para reutilizar:** World Monitor (el "worldmonitor" que se busca) es **AGPL-3.0**: sirve como **referencia** de qué fuentes usar (usa aisstream, OpenSky y GDELT), no para copiar código. OpenBB v5 es **Apache-2.0**. Para gráficos, lightweight-charts y KLineChart son Apache-2.0.
7. **Formación con evidencia:** Aronson, Carver, Chan, Kaufman, López de Prado, Lo, Harris, y cursos abiertos del MIT y de Yale. Nada de influencers.

El top 10 ordenado por valor y esfuerzo está al final (§6).

---

## 1. Qué ofrecen las terminales profesionales

Según las páginas de producto. No se probó ninguna con cuenta.

| Función | Bloomberg | LSEG Workspace | Koyfin | TradingView | Sierra Chart / ATAS | ¿La replicamos con datos gratis? |
|---|---|---|---|---|---|---|
| Noticias en tiempo real de todo el mundo | Sí (propias + miles de fuentes) | Sí (Reuters) | Parcial | Titulares de terceros | No | **Parcial:** GDELT + bancos centrales + organismos oficiales. Sin Reuters ni Bloomberg (licencia de pago). |
| Calendario económico y sorpresas | Sí | Sí | Sí | Sí | No | **Sí**, ya existe (ADR 0010, 0016, 0018). |
| Macro (tasas, inflación, empleo, curva) | Sí | Sí | Sí | Parcial | No | **Sí:** BCE, Eurostat, BLS, Tesoro (ADR 0006, 0012, 0015, 0016). FRED no permite guardar. |
| Energía y materias primas | Sí | Sí | Parcial | Precios | Precios | **Sí** con EIA (dominio público). |
| Mapa de buques y flujos de petróleo | Sí (mapas de buques) | Sí (flujos de materias primas) | No | No | No | **Parcial:** aisstream en vivo (términos sin publicar). Sin historia global gratis. |
| Vuelos | Sí (función de mapas) | No documentado | No | No | No | **Sí** para uso personal de investigación (OpenSky, no comercial). |
| Documentos de empresas (resultados) | Sí | Sí | Sí | Parcial | No | **Sí:** SEC EDGAR gratis (falta email de contacto, ADR 0006). |
| Gráficos e indicadores | Sí | Sí | Sí | Su punto fuerte | Sí | **Sí**, ya existe en el motor y en la UI. |
| Replay vela a vela | No es su foco | No es su foco | No | Sí ("Bar Replay") | Sí | **Sí, con los datos que ya tenemos.** |
| Flujo de órdenes (footprint, DOM, delta, perfil de volumen) | Parcial | Parcial | No | Parcial | **Su punto fuerte** | **Solo en cripto gratis** (libro de órdenes público). En futuros CME/Eurex el dato por operación es caro. |
| Backtest y laboratorio | Sí (BQuant) | Sí (Python) | No | Pine Script | Sí | **Sí, ya existe** (motor + walk-forward, Monte Carlo, PBO). |
| Chat con analistas o IA | Sí | Sí | No | No | No | **Sí**, con el modelo local (ADR 0008). |

Fuentes: [Bloomberg Terminal](https://www.bloomberg.com/professional/products/bloomberg-terminal/), [LSEG Workspace](https://www.lseg.com/en/data-analytics/products/workspace), [Koyfin precios](https://www.koyfin.com/pricing/), [TradingView precios](https://www.tradingview.com/pricing/), [Sierra Chart](https://www.sierrachart.com/), [ATAS](https://atas.net/).

**Conclusión:** lo que hace "profesional" a una terminal no son los datos de pago, sino **densidad, velocidad y disciplina** (todo con hora de publicación, fuente y contexto en una pantalla). Eso se puede copiar. Lo que no se puede copiar gratis: noticias de agencias (Reuters, Bloomberg, Dow Jones), historia global de buques y el flujo de órdenes completo de futuros.

---

## 2. Proyectos de código abierto

Licencia y actividad leídas con la API de GitHub el 26-11-2026.

| Proyecto | Licencia | Actividad | Qué reutilizar | Ojo |
|---|---|---|---|---|
| [World Monitor](https://github.com/koala73/worldmonitor) | **AGPL-3.0** | ~91 000 estrellas, último push 25-11-2026 | **Ideas y catálogo de fuentes**: usa aisstream (`shared/ais-watchdog.js`), OpenSky (`api/opensky.js`) y GDELT (`src/services/gdelt-intel.ts`). Mapa con deck.gl + MapLibre. | AGPL: copiar código obliga a publicar el nuestro bajo AGPL. Usarlo solo como referencia. También pide no presentar copias como el sitio oficial. |
| [OpenBB](https://github.com/OpenBB-finance/OpenBB) | **Apache-2.0** desde v5 ([FAQ de licencia](https://docs.openbb.co/odp/python/faqs/license)). v4 y anteriores eran AGPL ([anuncio](https://openbb.co/blog/license-change-openbb-platform-goes-agpl/)). | ~76 000 estrellas, push 25-11-2026 | Conectores de datos en Python como **referencia** de endpoints oficiales. | La API de GitHub redirige el repo a `openbq-org/OpenBB`: **verificar** que sea la organización oficial antes de clonar. Muchos de sus conectores son a proveedores con términos propios. |
| [lightweight-charts](https://github.com/tradingview/lightweight-charts) | Apache-2.0 | ~18 000 estrellas, push 18-11-2026 | Gráfico de velas rápido en el navegador. | Pide atribución a TradingView (archivo NOTICE). Hoy la UI usa un gráfico propio: cambiarlo es una "librería principal" y va con ADR. |
| [KLineChart](https://github.com/klinecharts/KLineChart) | Apache-2.0 | ~4 300 estrellas, push 16-11-2026 | Alternativa de gráfico con dibujos. | Misma regla: ADR si se adopta. |
| [Lean (QuantConnect)](https://github.com/QuantConnect/Lean) | Apache-2.0 | ~22 700 estrellas, push 25-11-2026 | Ideas de diseño de backtest y de reportes. | C#. No reemplaza al motor en Rust. |
| [nautilus_trader](https://github.com/nautechsystems/nautilus_trader) | LGPL-3.0 | ~31 800 estrellas, push 26-11-2026 | Referencia de motor en Rust con replay por eventos. | No duplicar el motor: solo ideas. |
| [pysystemtrade](https://github.com/pst-group/pysystemtrade) (de Rob Carver) | GPL-3.0 | ~3 600 estrellas, push 02-11-2026 | Material del curso: reglas de tendencia y carry explicadas por el autor del libro. | GPL. Python no recalcula indicadores (`CLAUDE.md` §1): solo como lectura. |
| [machine-learning-for-trading](https://github.com/stefan-jansen/machine-learning-for-trading) | MIT | ~22 000 estrellas, push 25-11-2026 | Cuadernos para el curso de ML. | |
| [vectorbt](https://github.com/polakowo/vectorbt) | Apache-2.0 + **Commons Clause** (no se puede vender) | ~10 000 estrellas | Solo referencia. | |
| [backtesting.py](https://github.com/kernc/backtesting.py) | AGPL-3.0 | ~9 300 estrellas | Solo referencia. | |

**Replay vela a vela y simuladores de prop firms:** en GitHub no aparece ningún proyecto maduro. Las búsquedas `bar replay trading` y `prop firm challenge simulator` devuelven repos con 0 o 1 estrella. **Conviene construirlos nosotros**, sobre el motor y el módulo de riesgo que ya existen.

---

## 3. Fuentes de datos gratis o baratas, por categoría

"Guardar" = si los términos permiten almacenar los datos en la base local. Cada fila nueva necesita su ADR antes de usarse.

### 3.1 Buques (AIS)

| Fuente | Costo | Clave | ¿Guardar? | Latencia | Historia | Fuente |
|---|---|---|---|---|---|---|
| **aisstream.io** | Gratis | Sí (inicio de sesión con GitHub) | **Sin verificar:** la página no publica términos ni licencia | Tiempo real por WebSocket. No da cifra. Si el cliente lee lento, descarta mensajes | No ofrece | [aisstream.io](https://aisstream.io/) |
| | | | Hasta 3 conexiones por IP y por cuenta. No permite conectar desde el navegador: necesita un servidor propio (el motor sirve). Dice usar una "red global de estaciones AIS", sin detallar la cobertura. | | | |
| **AISHub** | Gratis | Solo si compartís el dato de **tu propio receptor AIS** (≥10 buques, ≥90 % de tiempo activo) | No se puede pasar el dato crudo a terceros. El almacenamiento no se menciona | API: 1 pedido por minuto | No documentado | [AISHub](https://www.aishub.net/join-us) |
| **Global Fishing Watch** | Gratis | Sí | "only available for non-commercial purposes". Pide citarlos | "Near real time" solo para eventos. Sin cifra | Sin verificar | [APIs](https://globalfishingwatch.org/our-apis/), [documentación](https://globalfishingwatch.org/our-apis/documentation) |
| | | | Útil para **visitas a puertos** y encuentros entre buques. Está centrado en la pesca: no confirma cobertura de petroleros. | | | |
| **MarineTraffic / VesselFinder** | De pago (créditos) | Sí | Sin verificar (la web rechazó la lectura automática) | | | [VesselFinder API](https://vesselfinder.com/realtime-ais-data) |

**Lectura:** un mapa de petroleros **en vivo** es posible gratis con aisstream. Un **historial global** de flujos de petróleo gratis no existe. Hay que construirlo guardando el stream, y eso depende de que aisstream lo permita por escrito. Para identificar petroleros se usa el tipo de buque del mensaje AIS (códigos 80–89 = tanqueros, según la norma ITU-R M.1371).

### 3.2 Energía, materias primas y posicionamiento

| Fuente | Costo | Clave | ¿Guardar? | Latencia | Historia | Fuente |
|---|---|---|---|---|---|---|
| **EIA (EE. UU.)** | Gratis | Sí (registro) | **Sí:** "U.S. government publications are in the public domain". Pide citar la fuente con la fecha | Archivos masivos 2 veces por día. Series semanales de crudo y gas (inventarios, producción, importaciones). Algunos precios diarios | Décadas | [API](https://www.eia.gov/opendata/), [reutilización](https://www.eia.gov/about/copyrights_reuse.php) |
| **CFTC Commitments of Traders** | Gratis | No | Dato del gobierno de EE. UU. **Sin verificar en esta ronda** | Semanal (los viernes, con datos del martes) | Desde los años 80 | [CFTC COT](https://www.cftc.gov/MarketReports/CommitmentsofTraders/index.htm) |
| **Oro y otras materias primas** | | | | | | Ya decidido en el [ADR 0013](../decisions/0013-materias-primas-oro.md) |

### 3.3 Vuelos

| Fuente | Costo | Clave | ¿Guardar? | Latencia | Historia | Fuente |
|---|---|---|---|---|---|---|
| **OpenSky Network** | Gratis | Opcional (OAuth2 de cliente; los tokens duran 30 min) | La API es "for research and non-commerical purposes". La licencia de datasets permite investigación y educación sin fines de lucro. La página oficial de términos rechazó la lectura (403): **sin verificar** | Estados cada 10 s (anónimo) o cada 5 s (con cuenta) | Con cuenta: estados de hasta 1 h atrás. Trayectorias hasta 30 días | [REST API](https://openskynetwork.github.io/opensky-api/rest.html), [copia de la licencia](https://zenodo.org/records/6325961/files/LICENSE.txt?download=1) |
| | | | Créditos por día: 400 anónimo, 4 000 con cuenta, 8 000 si aportás un receptor. | | | |

**Lectura:** los vuelos aportan poco al trading de índices. Solo valen como contexto visual (por ejemplo, cierres de espacio aéreo).

### 3.4 Macro y bancos centrales

| Fuente | Costo | ¿Guardar? | Estado en el repo | Fuente |
|---|---|---|---|---|
| **BCE (estadísticas)** | Gratis | Reutilización libre, también comercial, citando "Source: ECB statistics" y sin modificar los datos | Ya en el [ADR 0006](../decisions/0006-fuentes-de-noticias.md) | [Política de uso](https://www.ecb.europa.eu/stats/ecb_statistics/governance_and_quality_framework/html/usage_policy.en.html) |
| **BLS, Fed (FOMC)** | Gratis | Dominio público (gobierno de EE. UU.) | ADR 0006 y 0016 | |
| **Tesoro de EE. UU. y NY Fed** | Gratis | | ADR 0012 y 0015 | |
| **Eurostat** | Gratis | **Sin verificar** (la página de copyright dio 404) | Nuevo | |
| **FRED** | Gratis | **No:** prohíbe "Store, cache, or archive" | Descartado en [fuentes-datos-gratis](fuentes-datos-gratis-2026-11.md) | [legal](https://fred.stlouisfed.org/legal/) |
| **Discursos de bancos centrales (BIS)** | Gratis | Se puede descargar y redistribuir solo para fines **no comerciales**. Fragmentos de hasta 400 palabras con cita | Nuevo. Los discursos de la Fed y del BCE ya están en el ADR 0006 | [Términos del BIS](https://www.bis.org/terms_conditions.htm) |

### 3.5 Empresas y resultados

| Fuente | Costo | Clave | ¿Guardar? | Latencia | Fuente |
|---|---|---|---|---|---|
| **SEC EDGAR** | Gratis | No, pero exige un *User-Agent* con nombre y email de contacto | Descarga libre. Máximo **10 pedidos por segundo** | Los documentos entran de 6:00 a 22:00 ET. JSON en `data.sec.gov` | [Acceso a EDGAR](https://www.sec.gov/search-filings/edgar-search-assistance/accessing-edgar-data) |

Sigue pendiente desde el ADR 0006: el email de contacto lo elige el dueño.

### 3.6 Noticias y eventos globales

| Fuente | Costo | ¿Guardar? | Latencia | Historia | Fuente |
|---|---|---|---|---|---|
| **GDELT** | Gratis | **Sí:** "unlimited and unrestricted use for any academic, commercial, or governmental use", y "You may redistribute, rehost, republish, and mirror". Exige citar a GDELT con un enlace | La página habla de actualizaciones diarias. El intervalo de 15 minutos de GDELT 2.0 **no se verificó** en esta ronda | Desde 1979 | [About](https://www.gdeltproject.org/about.html) |
| **RSS de medios** (Reuters, FT, El País, etc.) | Gratis para leer | En general **no**: los términos de cada medio limitan la reproducción. El [ADR 0006](../decisions/0006-fuentes-de-noticias.md) dejó fuera los medios privados y prohíbe el scraping | Minutos | | |

**Lectura:** GDELT da el "qué pasa en el mundo" (eventos, tono, países, temas) con permiso de guardar, y enlaza a la noticia original. **La noticia completa no se guarda:** se muestra el titular y el enlace. Regla de hora real (`CLAUDE.md` §5b): GDELT trae la fecha en que **lo vio**, que sirve como hora de disponibilidad.

### 3.7 Cripto como mercado de práctica 24/7

| Fuente | Costo | Clave | ¿Guardar? | Qué da | Fuente |
|---|---|---|---|---|---|
| **Coinbase Exchange (WebSocket público)** | Gratis | No | Rige su "Market Data Terms of Use". **No se leyeron**: sin verificar | Operaciones y **libro de órdenes** en tiempo real | [WebSocket](https://docs.cdp.coinbase.com/exchange/websocket-feed/overview) |
| **Binance (`data.binance.vision`)** | Gratis | No | Según su FAQ, "strictly follow the Binance Terms of Use". Sin licencia explícita: **sin verificar** | Velas y operaciones históricas en archivos diarios o mensuales | [FAQ](https://www.binance.com/en-BH/support/faq/how-to-download-historical-market-data-on-binance-5810ae42176b4770b880ce1f14932262), [repo](https://github.com/binance/binance-public-data/) |
| **Kraken** | | | Página de términos de datos: 404, sin verificar | | |

**Lectura:** cripto es el único mercado donde el **flujo de órdenes** (libro, delta, footprint al estilo ATAS) es gratis. Sirve para **aprender** order flow, aunque no sea el mercado de los CFD. Solo datos: nada de cuentas ni órdenes en un exchange.

### 3.8 Datos de mercado que ya tenemos

| Fuente | Qué da | Límite | ADR |
|---|---|---|---|
| IG (demo) | Velas de los CFD | 10 000 puntos de historia por semana | [0002](../decisions/0002-broker-ig.md) |
| IBKR (paper) | Datos diferidos, futuros micro | Ver ADR | [0017](../decisions/0017-ibkr-paper-futuros-micro.md) |
| Databento | Velas de 1 minuto de CME desde 2011 y de Eurex desde el 10-03-2025 | De pago por GB (unos USD 65/GB en CME). El dato por operación (para order flow) cuesta bastante más | [0011](../decisions/0011-historico-intradia-databento.md) |

Con esto ya alcanza para el **replay** y los **exámenes**: varios años de velas de 1 minuto guardadas en local.

---

## 4. Formación con evidencia

Criterio: autores con trabajo publicado y revisable (libros de editoriales académicas o técnicas, papers, cursos universitarios). Nada de influencers.

### Libros (orden sugerido para el curso)

| Nivel | Libro | Por qué |
|---|---|---|
| 1 | Larry Harris, *Trading and Exchanges* (Oxford University Press, 2003) | Cómo funciona un mercado: órdenes, liquidez, quién gana y quién pierde. Base para el order flow. |
| 1 | Andrew Lo, *Adaptive Markets* (Princeton University Press, 2017) | Por qué las estrategias dejan de funcionar. |
| 2 | David Aronson, *Evidence-Based Technical Analysis* (Wiley, 2006) | Cómo probar si una regla técnica funciona de verdad, y el sesgo de minería de datos. |
| 2 | Robert Carver, *Systematic Trading* (2015), *Leveraged Trading* (2019) y *Advanced Futures Trading Strategies* (2023), Harriman House | Sistemas simples y robustos, tamaño de posición y costos. Su código es pysystemtrade. |
| 2 | Perry Kaufman, *Trading Systems and Methods* (Wiley, 6.ª ed.) | Catálogo de métodos, con pruebas. |
| 3 | Ernest Chan, *Quantitative Trading* (2.ª ed.) y *Algorithmic Trading* (Wiley) | Reversión a la media, momentum y backtest práctico. |
| 3 | Marcos López de Prado, *Advances in Financial Machine Learning* (Wiley, 2018) | Validación cruzada purgada, sobreajuste y meta-etiquetado. Ya inspira el PBO del laboratorio (#664). |

### Papers

- Bailey, Borwein, López de Prado y Zhu, "The Probability of Backtest Overfitting" ([SSRN 2326253](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2326253)): base del PBO.
- Harvey, Liu y Zhu, "…and the Cross-Section of Expected Returns" ([NBER w20592](https://www.nber.org/papers/w20592)): por qué hay que subir la vara estadística cuando se prueban muchas ideas.
- Chague, De-Losso y Giovannetti, "Day Trading for a Living?" ([SSRN 3423101](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=3423101)): de quienes hicieron day trading en minicontratos del Ibovespa más de 300 días, el 97 % perdió dinero y solo el 1,1 % ganó más que el salario mínimo. No encontraron evidencia de que los traders mejoren con el tiempo.

### Cursos abiertos

- MIT OpenCourseWare 18.S096, [Topics in Mathematics with Applications in Finance](https://ocw.mit.edu/courses/18-s096-topics-in-mathematics-with-applications-in-finance-fall-2013/).
- Open Yale ECON 252, [Financial Markets](https://oyc.yale.edu/economics/econ-252) (Robert Shiller).

### Cuentas fondeadas y CFD: los números publicados

| Dato | Valor | Fuente |
|---|---|---|
| Evaluaciones de Topstep aprobadas en 2025 (por intento) | **16,8 %** | [Divulgación de riesgos de Topstep](https://www.topstep.com/risk-disclosure) |
| Personas que aprobaron al menos una evaluación (2025) | 51,8 % | ídem |
| Personas en cuenta fondeada que cobraron algún pago (2025) | **33,3 %** | ídem |
| Personas con cuenta "Express Funded" llamadas a una cuenta real | **0,71 %** | ídem |
| Cuentas minoristas de CFD en la UE que pierden dinero | **74 %–89 %** | [ESMA, aviso de intervención de 2018](https://www.esma.europa.eu/sites/default/files/library/esma35-43-1135_notice_of_pi_decisions_on_cfds_and_binary_options.pdf) |
| FTMO | **No publica** su tasa de aprobación. El "~10 %" que circula sale de entrevistas, no de un dato oficial | blogs del sector, sin fuente primaria |

Topstep aclara que sus cifras mezclan entorno simulado y real y que no predicen resultados futuros. **El negocio de las prop firms vive sobre todo de las cuotas de evaluación.** El módulo educativo tiene que mostrarlo así.

---

## 5. Riesgos y supuestos

- **aisstream, Coinbase, Binance, Kraken y Eurostat:** términos **sin verificar**. No se guarda nada de ellos hasta tener los términos por escrito (en el ADR).
- **Licencias copyleft (AGPL, GPL):** World Monitor, backtesting.py y pysystemtrade solo como referencia. No se copia código.
- **Regla de un solo lugar** (`CLAUDE.md` §1): los indicadores y el order flow (delta, footprint) se calculan **solo en Rust**. Las librerías de Python de la tabla 2 no se usan para recalcular.
- **Hora real:** cada fuente nueva necesita su hora de disponibilidad (`CLAUDE.md` §5b). Los datos de AIS y de vuelos llegan casi en vivo, pero sin historia no sirven para backtest.
- **Sin dinero real ni consejo personalizado:** el curso y el simulador de prop firms son educativos.
- **Cuentas:** aisstream, EIA, OpenSky y Global Fishing Watch piden cuenta o clave. Las crea el dueño, si las aprueba.

---

## 6. Top 10 recomendaciones (valor / esfuerzo)

| # | Recomendación | Valor | Esfuerzo | ¿Qué hace falta? |
|---|---|---|---|---|
| 1 | **Replay vela a vela y exámenes prácticos** con las velas de IG y Databento ya guardadas (ocultar el futuro, decidir, revelar, puntuar) | Muy alto: es el corazón de la academia | Medio | **Sin ADR de datos** (ADR 0002 y 0011). Contrato nuevo en `contracts/` y spec de la academia aprobada por el dueño. |
| 2 | **Simulador de reglas de prop firms** (pérdida diaria, drawdown máximo, objetivo, consistencia) sobre el módulo de riesgo, con los números reales de §4 | Alto | Bajo-medio | Sin ADR de datos. Las reglas de cada firma van en `config/`, citadas desde la página oficial de cada firma. Spec aprobada por el dueño. |
| 3 | **Biblioteca y ruta del curso** con las fuentes de §4 (solo referencias y resúmenes propios, sin copiar texto) | Alto | Bajo | Sin ADR. |
| 4 | **GDELT** como feed de eventos y noticias globales (titular + enlace + tono + país) | Alto | Medio | **ADR nuevo** de proveedor de noticias (amplía el 0006). |
| 5 | **Panel de energía con la EIA** (inventarios semanales de crudo y gas, producción) en el calendario y como contexto | Medio-alto | Bajo | **ADR nuevo** (proveedor). Dominio público. El dueño crea la clave. |
| 6 | **Mapa de petroleros en vivo con aisstream** (solo vista; guardar recién cuando lo permitan por escrito) | Medio (muy visual, poco poder predictivo probado) | Medio | **ADR nuevo** (proveedor + términos pedidos por escrito). El dueño crea la cuenta. |
| 7 | **Rediseño "terminal densa"** (sin vidrio): paneles, atajos de teclado, todo con hora y fuente | Alto en uso diario | Medio | Sin ADR. Si se cambia la librería de gráficos (lightweight-charts o KLineChart), **ADR** de librería principal. |
| 8 | **Cripto como mercado de práctica 24/7 y para aprender order flow** (libro de órdenes público; delta y footprint calculados en Rust) | Medio | Medio-alto | **ADR nuevo** (mercado y proveedor nuevos), después de leer los términos de datos de Coinbase o Binance. Solo datos, sin cuenta en un exchange. |
| 9 | **Macro ampliada:** CFTC COT, Eurostat y discursos (BIS, solo uso no comercial) | Medio | Bajo | **ADR nuevo** (amplía el 0006), con los términos de Eurostat y la CFTC verificados. |
| 10 | **SEC EDGAR** (resultados de las empresas del NASDAQ 100) y **OpenSky** como capa visual opcional | Bajo-medio | Bajo | EDGAR: cerrar el pendiente del ADR 0006 (el dueño elige el email). OpenSky: **ADR nuevo**, uso no comercial. |

**Lo que no se recomienda:** scraping de MarineTraffic, Bloomberg, Reuters o Investing (términos), FRED como almacén (lo prohíbe), y copiar código AGPL o GPL dentro del repo.
