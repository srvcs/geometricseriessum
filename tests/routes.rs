use axum::body::Body;
use axum::extract::Json as AxumJson;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::{Json, Router as AxumRouter};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use srvcs_geometricseriessum::{api::Deps, health, router, telemetry};
use tower::ServiceExt;

const DEAD_URL: &str = "http://127.0.0.1:1";

/// Spawn a *computing* mock `srvcs-power`: reads `{"base": b, "exp": e}` and
/// returns `{"result": b^e}` (integer exponentiation). The orchestration is
/// genuinely driven by this answer rather than a canned value.
async fn spawn_power() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let base = body.get("base").and_then(Value::as_i64).unwrap_or(0);
            let exp = body.get("exp").and_then(Value::as_i64).unwrap_or(0);
            let mut acc: i64 = 1;
            for _ in 0..exp.max(0) {
                acc *= base;
            }
            Json(json!({ "result": acc }))
        }),
    );
    serve(app).await
}

/// Spawn a *computing* mock `srvcs-multiply`: returns `{"result": a * b}`.
async fn spawn_multiply() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_i64).unwrap_or(0);
            let b = body.get("b").and_then(Value::as_i64).unwrap_or(0);
            Json(json!({ "result": a * b }))
        }),
    );
    serve(app).await
}

/// Spawn a *computing* mock `srvcs-subtract`: returns `{"result": a - b}`.
async fn spawn_subtract() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_i64).unwrap_or(0);
            let b = body.get("b").and_then(Value::as_i64).unwrap_or(0);
            Json(json!({ "result": a - b }))
        }),
    );
    serve(app).await
}

/// Spawn a *computing* mock `srvcs-divide`: returns `{"result": a / b}`
/// (integer division), or `422` on divide-by-zero.
async fn spawn_divide() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_i64).unwrap_or(0);
            let b = body.get("b").and_then(Value::as_i64).unwrap_or(1);
            if b == 0 {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({ "error": "divide by zero" })),
                );
            }
            (StatusCode::OK, Json(json!({ "result": a / b })))
        }),
    );
    serve(app).await
}

/// Spawn a mock returning a fixed status + body (used for error-path tests).
async fn spawn_fixed(status: StatusCode, body: Value) -> String {
    let app = AxumRouter::new().route(
        "/",
        post(move || {
            let body = body.clone();
            async move { (status, Json(body)) }
        }),
    );
    serve(app).await
}

async fn serve(app: AxumRouter) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

struct Urls {
    power: String,
    multiply: String,
    subtract: String,
    divide: String,
}

/// All four dependencies live and computing.
async fn live_urls() -> Urls {
    Urls {
        power: spawn_power().await,
        multiply: spawn_multiply().await,
        subtract: spawn_subtract().await,
        divide: spawn_divide().await,
    }
}

fn app(urls: &Urls) -> axum::Router {
    router(
        telemetry::metrics_handle_for_tests(),
        Deps {
            power_url: urls.power.clone(),
            multiply_url: urls.multiply.clone(),
            subtract_url: urls.subtract.clone(),
            divide_url: urls.divide.clone(),
        },
    )
}

async fn gss(urls: &Urls, first: i64, ratio: i64, n: i64) -> (StatusCode, Value) {
    let res = app(urls)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "first": first, "ratio": ratio, "n": n }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn dead_urls() -> Urls {
    Urls {
        power: DEAD_URL.to_string(),
        multiply: DEAD_URL.to_string(),
        subtract: DEAD_URL.to_string(),
        divide: DEAD_URL.to_string(),
    }
}

async fn status_of(uri: &str) -> StatusCode {
    app(&dead_urls())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

// --- Standard endpoints. ---

#[tokio::test]
async fn healthz_ok() {
    assert_eq!(status_of("/healthz").await, StatusCode::OK);
}

#[tokio::test]
async fn readyz_reflects_state() {
    health::set_ready(true);
    assert_eq!(status_of("/readyz").await, StatusCode::OK);
}

#[tokio::test]
async fn metrics_ok() {
    assert_eq!(status_of("/metrics").await, StatusCode::OK);
}

#[tokio::test]
async fn openapi_ok() {
    assert_eq!(status_of("/openapi.json").await, StatusCode::OK);
}

#[tokio::test]
async fn generates_request_id_when_absent() {
    let res = app(&dead_urls())
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        res.headers().contains_key("x-request-id"),
        "response must carry a generated x-request-id"
    );
}

#[tokio::test]
async fn index_reports_identity() {
    let res = app(&dead_urls())
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["service"], "srvcs-geometricseriessum");
    assert_eq!(
        body["concern"],
        "sequences: sum of first n terms of a geometric sequence"
    );
    assert_eq!(
        body["depends_on"],
        json!([
            "srvcs-power",
            "srvcs-multiply",
            "srvcs-subtract",
            "srvcs-divide"
        ])
    );
}

// --- Correctness cases, against the computing mocks. ---

#[tokio::test]
async fn gss_1_2_4_is_15() {
    let urls = live_urls().await;
    let (status, body) = gss(&urls, 1, 2, 4).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["first"], 1);
    assert_eq!(body["ratio"], 2);
    assert_eq!(body["n"], 4);
    // rn=16; num=15; num2=15; den=1; 15/1=15  (1+2+4+8)
    assert_eq!(body["result"], 15);
}

#[tokio::test]
async fn gss_3_2_3_is_21() {
    let urls = live_urls().await;
    let (status, body) = gss(&urls, 3, 2, 3).await;
    assert_eq!(status, StatusCode::OK);
    // 3 + 6 + 12 = 21; rn=8; num=7; num2=21; den=1; 21/1=21
    assert_eq!(body["result"], 21);
}

#[tokio::test]
async fn gss_2_3_4_is_80() {
    let urls = live_urls().await;
    let (status, body) = gss(&urls, 2, 3, 4).await;
    assert_eq!(status, StatusCode::OK);
    // 2 + 6 + 18 + 54 = 80; rn=81; num=80; num2=160; den=2; 160/2=80
    assert_eq!(body["result"], 80);
}

#[tokio::test]
async fn gss_ratio_one_is_first_times_n() {
    // ratio == 1 path: every term equals `first`; sum = first * n via multiply.
    // Point power/subtract/divide at dead ports to prove they are never called.
    let urls = Urls {
        power: DEAD_URL.to_string(),
        multiply: spawn_multiply().await,
        subtract: DEAD_URL.to_string(),
        divide: DEAD_URL.to_string(),
    };
    let (status, body) = gss(&urls, 5, 1, 4).await;
    assert_eq!(status, StatusCode::OK);
    // 5 + 5 + 5 + 5 = 20
    assert_eq!(body["result"], 20);
}

#[tokio::test]
async fn gss_n_one_is_first() {
    let urls = live_urls().await;
    let (status, body) = gss(&urls, 7, 5, 1).await;
    assert_eq!(status, StatusCode::OK);
    // rn=5; num=4; num2=28; den=4; 28/4=7
    assert_eq!(body["result"], 7);
}

// --- Error / degraded paths. ---

#[tokio::test]
async fn degrades_when_power_unreachable() {
    let urls = Urls {
        power: DEAD_URL.to_string(),
        multiply: spawn_multiply().await,
        subtract: spawn_subtract().await,
        divide: spawn_divide().await,
    };
    let (status, body) = gss(&urls, 1, 2, 4).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["dependency"], "srvcs-power");
}

#[tokio::test]
async fn degrades_when_subtract_unreachable() {
    // power reachable, so the pipeline reaches the first subtract call.
    let urls = Urls {
        power: spawn_power().await,
        multiply: spawn_multiply().await,
        subtract: DEAD_URL.to_string(),
        divide: spawn_divide().await,
    };
    let (status, body) = gss(&urls, 1, 2, 4).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["dependency"], "srvcs-subtract");
}

#[tokio::test]
async fn degrades_when_multiply_unreachable() {
    // power + subtract reachable, so the pipeline reaches the multiply call.
    let urls = Urls {
        power: spawn_power().await,
        multiply: DEAD_URL.to_string(),
        subtract: spawn_subtract().await,
        divide: spawn_divide().await,
    };
    let (status, body) = gss(&urls, 1, 2, 4).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["dependency"], "srvcs-multiply");
}

#[tokio::test]
async fn degrades_when_divide_unreachable() {
    // power + subtract + multiply reachable, so the pipeline reaches divide.
    let urls = Urls {
        power: spawn_power().await,
        multiply: spawn_multiply().await,
        subtract: spawn_subtract().await,
        divide: DEAD_URL.to_string(),
    };
    let (status, body) = gss(&urls, 1, 2, 4).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["dependency"], "srvcs-divide");
}

#[tokio::test]
async fn forwards_422_from_power() {
    let urls = Urls {
        power: spawn_fixed(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "value is not an integer" }),
        )
        .await,
        multiply: spawn_multiply().await,
        subtract: spawn_subtract().await,
        divide: spawn_divide().await,
    };
    let (status, _) = gss(&urls, 1, 2, 4).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn forwards_422_from_multiply_on_ratio_one() {
    // ratio == 1 routes straight to multiply; a 422 there is forwarded.
    let urls = Urls {
        power: DEAD_URL.to_string(),
        multiply: spawn_fixed(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "bad operand" }),
        )
        .await,
        subtract: DEAD_URL.to_string(),
        divide: DEAD_URL.to_string(),
    };
    let (status, _) = gss(&urls, 1, 1, 4).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn forwards_422_from_divide() {
    // den would be zero only if ratio==1, which short-circuits earlier; force a
    // 422 from the divide mock directly to exercise the forward path.
    let urls = Urls {
        power: spawn_power().await,
        multiply: spawn_multiply().await,
        subtract: spawn_subtract().await,
        divide: spawn_fixed(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "divide by zero" }),
        )
        .await,
    };
    let (status, _) = gss(&urls, 1, 2, 4).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn malformed_power_result_is_500() {
    // power answers 200 but with no integer result -> contract violation -> 500.
    let urls = Urls {
        power: spawn_fixed(StatusCode::OK, json!({ "result": "not-a-number" })).await,
        multiply: spawn_multiply().await,
        subtract: spawn_subtract().await,
        divide: spawn_divide().await,
    };
    let (status, body) = gss(&urls, 1, 2, 4).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["dependency"], "srvcs-power");
}
