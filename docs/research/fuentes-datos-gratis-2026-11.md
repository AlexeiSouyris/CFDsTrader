# Fuentes de precios gratuitas para no depender del cupo de IG (noviembre 2026)

Pedido del dueño (21 y 22-11-2026): datos **gratis**, que corran **localmente**, para que el avance no dependa de los 10.000 puntos por semana de IG. Se acepta operar en otro broker y alguna diferencia de precio o de tiempo.

Índices: Alemania 40 (DE40), US Tech 100, US 500 y Wall Street 30. Residencia: **España**.

Todo se leyó en páginas oficiales en esas fechas. Lo que no estaba documentado queda como "no documentado", no supuesto. No es una decisión: cambiar de proveedor requiere un ADR aprobado por las dos personas (`CLAUDE.md` §6).

## Resultado

| Fuente | Qué se encontró | Veredicto |
|---|---|---|
| **IG, streaming** (Lightstreamer) | Conecta (`demo-apd.marketdatasystems.com`), pero `CHART:{epic}:TICK` y `CHART:{epic}:1MINUTE` responden "Insufficient permissions". `PRICE:{cuenta}:{epic}` y `MARKET:{epic}` responden "Invalid group". Probado desde la nube (20-11) y desde el Mac mini (22-11), sin gastar cupo. La documentación da 40 suscripciones por conexión y una sola conexión. Que el streaming no cuente para el cupo no está dicho de forma explícita: el cupo "Applies to price history endpoints" ([FAQ](https://labs.ig.com/faq.html)). | ❌ No disponible en la demo. Se puede preguntar al soporte de la API de IG si lo habilitan. |
| **Capital.com** (API con demo) | Su página para España: "CFDs are not available to new clients residing in Spain". Su soporte respondió por escrito que las cuentas demo no están disponibles para residentes en España desde el 9-03-2026. | ❌ |
| **IC Markets EU** (cTrader y MetaTrader) | Su FAQ: "Según el comunicado de prensa de la CNMV, los clientes españoles no son elegibles para una cuenta demo" ([FAQ](https://icmarkets.eu/ss/recursos-ayuda/preguntas-frecuentes-faqs)). | ❌ en demo. |
| **cTrader Open API** (Spotware) | Gratis, demo y real, precios y velas en vivo, 5 pedidos de historia por segundo, sin cupo semanal documentado. Necesita que Spotware apruebe la aplicación ([docs](https://help.ctrader.com/open-api/)). | Depende de un broker que dé cuenta a residentes en España. Con IC Markets, solo con cuenta **real** (ver abajo). |
| **MetaTrader 5** | En macOS corre dentro de Wine. El paquete de Python solo tiene versiones para Windows ([PyPI](https://pypi.org/project/MetaTrader5/)). Los datos se sacarían con un programa en MQL5 dentro del terminal (`CopyRates`, archivos CSV o sockets). | Plan B: frágil y suma un cuarto lenguaje. |
| **OANDA** (v20) | "available to all divisions except OANDA Global Markets and OANDA TMS BROKERS S.A." ([intro](https://developer.oanda.com/rest-live-v20/introduction/)), y OANDA TMS es la entidad que atiende a la UE. | ❌ |
| **Dukascopy** | Exportación gratuita desde la web, pero sus términos prohíben bots y "construct a database of any kind" ([términos](https://www.dukascopy.com/swiss/english/legal-pages/terms-of-use/)). | ❌ |
| **FRED** | S&P 500, Dow y Nasdaq 100: solo cierre diario, S&P y Dow limitados a 10 años, no tiene DAX. Sus términos prohíben "Store, cache, or archive" ([legal](https://fred.stlouisfed.org/legal/)). | ❌ |
| **Alpaca, Twelve Data, Alpha Vantage** | En sus planes gratuitos: solo acciones y ETFs de EE. UU. (SPY, QQQ, DIA). No hay índices ni DAX. Alpha Vantage da 25 pedidos por día. | ❌ para este universo. |
| **Stooq** | Sus páginas no se pudieron leer de forma automática (necesitan JavaScript). Términos no verificados. | Sin verificar. |

## Conclusión (22-11-2026)

- La regla de la CNMV deja a los residentes en España sin cuentas demo nuevas en los brokers europeos consultados. **La demo de IG que ya tenemos es, en la práctica, la única fuente de precios de CFD gratuita disponible.**
- Recomendación: seguir con IG, cargar la historia **una sola vez** y en el Mac mini, y ordenar el uso del cupo (`docs/guides/trabajo-en-paralelo.md`).
- Alternativa a evaluar en un ADR: una cuenta **real** (sin operar) solo para leer datos por la API de cTrader. Falta verificar si una cuenta sin fondos recibe precios. Además, es una credencial con permiso para operar con dinero real (`CLAUDE.md` §5).
