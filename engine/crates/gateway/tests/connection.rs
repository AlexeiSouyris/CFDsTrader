//! #576: que el cliente cierre la conexión antes de que el gateway termine (el navegador o el
//! proxy de Vite cancelan un pedido, o se cierra un `EventSource`) es lo esperable, no un
//! `connection_error`. Se prueba con un servidor hyper de verdad sobre un socket local.

use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use cfds_gateway::ops::{client_closed, connection_end};
use cfds_obs::Level;
use hyper::body::{Bytes, Frame, Incoming};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Un body como el del SSE: manda un mensaje y después no termina nunca.
struct Endless(bool);

impl hyper::body::Body for Endless {
    type Data = Bytes;
    type Error = Infallible;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        if self.0 {
            return Poll::Pending;
        }
        self.0 = true;
        Poll::Ready(Some(Ok(Frame::data(Bytes::from_static(b"data: {}\n\n")))))
    }
}

/// Atiende una conexión con `slow` (tarda en contestar) o con un body sin fin, y devuelve cómo
/// terminó la conexión después de que el cliente hace `client`.
async fn serve_one<F, Fut>(slow: bool, client: F) -> Result<(), hyper::Error>
where
    F: FnOnce(TcpStream) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let service = service_fn(move |_req: Request<Incoming>| async move {
            if slow {
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
            Ok::<_, Infallible>(Response::new(Endless(false)))
        });
        hyper::server::conn::http1::Builder::new()
            .serve_connection(TokioIo::new(stream), service)
            .await
    });
    client(TcpStream::connect(addr).await.unwrap()).await;
    tokio::time::timeout(Duration::from_secs(10), server)
        .await
        .expect("la conexión no terminó")
        .unwrap()
}

const GET: &[u8] = b"GET /api/v1/ops/events HTTP/1.1\r\nHost: localhost\r\n\r\n";

#[tokio::test]
async fn a_request_cancelled_before_the_answer_is_the_client_closing() {
    // El proxy de Vite corta el pedido al gateway cuando el navegador lo cancela.
    let err = serve_one(true, |mut s| async move {
        s.write_all(GET).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(s);
    })
    .await
    .unwrap_err();
    assert!(err.is_incomplete_message(), "{err}");
    assert_eq!(
        err.to_string(),
        "connection closed before message completed"
    );
    assert!(client_closed(&err));
    assert_eq!(connection_end(&err), (Level::Info, "client_closed"));
}

#[tokio::test]
async fn closing_an_event_stream_is_the_client_closing() {
    // El `EventSource` de la vela en formación (#558) se cierra al cambiar de instrumento.
    let err = serve_one(false, |mut s| async move {
        s.write_all(GET).await.unwrap();
        let mut buf = [0u8; 256];
        let n = s.read(&mut buf).await.unwrap();
        assert!(n > 0);
        drop(s);
    })
    .await
    .unwrap_err();
    assert!(client_closed(&err), "{err}");
    assert_eq!(connection_end(&err).0, Level::Info);
}

#[tokio::test]
async fn a_malformed_request_is_still_a_connection_error() {
    let res = serve_one(false, |mut s| async move {
        s.write_all(b"esto no es http\r\n\r\n").await.unwrap();
        let mut buf = Vec::new();
        let _ = s.read_to_end(&mut buf).await;
    })
    .await;
    let err = res.unwrap_err();
    assert!(!client_closed(&err), "{err}");
    assert_eq!(connection_end(&err), (Level::Warn, "connection_error"));
}
