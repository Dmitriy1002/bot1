use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use crate::metrics::encode_metrics;

async fn metrics_handler(_req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "text/plain")
        .body(Body::from(encode_metrics()))
        .unwrap())
}

pub async fn start_metrics_server() {
    let addr = ([0, 0, 0, 0], 9898).into();
    let make_svc = make_service_fn(|_conn| async {
        Ok::<_, hyper::Error>(service_fn(metrics_handler))
    });

    if let Err(e) = Server::bind(&addr).serve(make_svc).await {
        eprintln!("Prometheus server error: {}", e);
    }
}