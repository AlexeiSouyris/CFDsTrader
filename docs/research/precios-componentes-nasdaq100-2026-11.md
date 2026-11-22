# Precios de los componentes del NASDAQ 100: verificación de fuentes (noviembre 2026)

Es la verificación que pide el ADR 0005 §2 (#10): una fuente de precios **diarios de 5 años**, que incluya las **empresas que salieron del índice** (sin sesgo de supervivencia), ajustados por splits y dividendos, y que se puedan **guardar localmente** para uso privado y no comercial. El dueño pidió fuentes **gratuitas** primero.

Todo se leyó en las páginas oficiales de cada proveedor el 22-11-2026. Lo que no estaba documentado queda como "no documentado". No se abrió ninguna cuenta ni se hizo ningún pedido a las API.

## Resultado

| Fuente | Plan gratuito | Empresas que dejaron de cotizar | Guardar los datos | Veredicto |
|---|---|---|---|---|
| **Tiingo** | "Starter", 0 USD: 1.000 pedidos por día, 50 por hora, 500 símbolos por mes. Historia de precios de 30+ años, con `adjClose` (método de CRSP) ([precios](https://www.tiingo.com/about/pricing), [EOD](https://www.tiingo.com/documentation/end-of-day)) | No documentado: la lista de tickers habla de la cobertura "actual" | **Prohibido en el plan gratuito**: "you may not write, save, archive, back up, or otherwise retain Tiingo Data in any persistent or durable storage", incluidos los dispositivos locales (§1.6(a)). En el plan pago (30 USD por mes para individuos) se puede guardar mientras esté activo, y al cancelar hay que **borrarlo todo** (§1.6(b)) ([términos](https://www.tiingo.com/tos)) | ❌ gratis. Pago: los datos no quedan nuestros |
| **EODHD** | 20 pedidos por día y **1 año** de historia: "only the past year of history and 20 API calls a day" ([EOD](https://eodhd.com/financial-apis/api-for-historical-data-and-volumes)) | Sí: precios diarios de empresas dadas de baja antes y después de 2018. La página no dice en qué planes ([delisted](https://eodhd.com/financial-apis/delisted-stock-companies-data-2)) | Permitido para un usuario no profesional: "store, manipulate, and analyze the data for private, non-commercial purposes". **Prohibido** "sharing access to their account with others, including within groups" ([términos](https://eodhd.com/financial-apis/terms-conditions)) | ❌ gratis (1 año). Pago: es el que mejor encaja, pero con una cuenta por persona |
| **Massive** (ex Polygon.io) | No se encontró un plan gratuito de acciones en la página de precios. El plan de acciones visible cuesta 199 USD por mes ([precios](https://massive.com/pricing)) | Sí: "use the All Tickers endpoint with active=false" ([acciones](https://massive.com/docs/rest/stocks/overview.md)) | Permitido para uso personal ("your own research, analysis and investment activities"), pero al terminar hay que borrar todo "within thirty (30) days" ([términos de datos](https://massive.com/legal/market-data-terms-of-service)) | ❌ gratis. Pago: caro, y los datos no quedan nuestros |
| **Financial Modeling Prep** | No verificado: sus páginas de precios, documentación y términos respondieron 403 a la lectura automática | — | — | Sin verificar: lo tiene que mirar una persona en el navegador |

## Conclusión

- **No hay una fuente gratuita** que cumpla las cuatro condiciones (5 años, empresas que dejaron de cotizar, ajustes, y poder guardarlo).
- Eso **no bloquea los análogos v1**: el ADR 0005 §4 ya los limita a los cuatro índices, con las velas diarias de IG.
- **Si algún día hace falta el universo del NASDAQ 100**, la opción paga que mejor encaja es **EODHD**: tiene empresas dadas de baja y sus términos permiten guardar los datos para uso privado. Hay que confirmar con EODHD qué plan incluye las dadas de baja y su precio, y tener en cuenta que no permite compartir la cuenta entre las dos personas. Es una decisión de las dos personas, con su costo.
- Tiingo y Massive pagos quedan detrás de EODHD: al dejar de pagar hay que borrar los datos, así que nunca quedan nuestros.
