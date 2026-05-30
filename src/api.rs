use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::{OpenApi, ToSchema};

use crate::client::{self, DepError};

pub const SERVICE: &str = "srvcs-geometricseriessum";
pub const CONCERN: &str = "sequences: sum of first n terms of a geometric sequence";
pub const DEPENDS_ON: &[&str] = &[
    "srvcs-power",
    "srvcs-multiply",
    "srvcs-subtract",
    "srvcs-divide",
];

/// Dependency endpoints, injected as router state so tests can point them at
/// mock services.
#[derive(Clone)]
pub struct Deps {
    pub power_url: String,
    pub multiply_url: String,
    pub subtract_url: String,
    pub divide_url: String,
}

#[derive(Serialize, ToSchema)]
pub struct Info {
    pub service: &'static str,
    pub concern: &'static str,
    pub depends_on: Vec<&'static str>,
}

/// `GET /` — service identity (srvcs service standard).
#[utoipa::path(get, path = "/", responses((status = 200, body = Info)))]
pub async fn index() -> Json<Info> {
    Json(Info {
        service: SERVICE,
        concern: CONCERN,
        depends_on: DEPENDS_ON.to_vec(),
    })
}

#[derive(Deserialize, ToSchema)]
pub struct EvalRequest {
    pub first: i64,
    pub ratio: i64,
    pub n: i64,
}

#[derive(Serialize, ToSchema)]
pub struct GeometricSeriesSumResponse {
    pub first: i64,
    pub ratio: i64,
    pub n: i64,
    pub result: i64,
}

fn ok(first: i64, ratio: i64, n: i64, result: i64) -> Response {
    (
        StatusCode::OK,
        Json(json!({ "first": first, "ratio": ratio, "n": n, "result": result })),
    )
        .into_response()
}

fn degraded(dependency: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "error": "dependency unavailable", "dependency": dependency })),
    )
        .into_response()
}

fn forward(status: u16, body: Value) -> Response {
    let code = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    (code, Json(body)).into_response()
}

/// A reachable dependency answered `200` but its body lacked an integer
/// `result`. That is a contract violation we cannot recover from, so surface a
/// `500` rather than guessing.
fn malformed(dependency: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(
            json!({ "error": "dependency returned a malformed result", "dependency": dependency }),
        ),
    )
        .into_response()
}

/// Call one dependency at `url` with `body`, mapping its outcome to either the
/// parsed response body (on `200`) or an early-return `Response` the caller
/// should surface verbatim:
///
/// - unreachable / non-`200`/`422` -> `503` degraded
/// - `422` -> forwarded `422` (the dependency rejected the input)
async fn ask(url: &str, body: &Value, dependency: &str) -> Result<Value, Response> {
    match client::call(url, body).await {
        Err(DepError::Unreachable) => Err(degraded(dependency)),
        Ok((200, body)) => Ok(body),
        Ok((422, body)) => Err(forward(422, body)),
        Ok(_) => Err(degraded(dependency)),
    }
}

/// `POST /` — compute the sum of the first `n` terms of the geometric sequence
/// with starting term `first` and common `ratio`, delegating every arithmetic
/// step to the dependency primitives:
///
/// - if `ratio == 1`, every term equals `first`, so the sum is `first * n`
///   (one `srvcs-multiply` call);
/// - otherwise apply the closed form `first * (ratio^n - 1) / (ratio - 1)`:
///   1. `rn = ratio^n`         via `srvcs-power`;
///   2. `num = rn - 1`         via `srvcs-subtract`;
///   3. `num2 = first * num`   via `srvcs-multiply`;
///   4. `den = ratio - 1`      via `srvcs-subtract`;
///   5. `result = num2 / den`  via `srvcs-divide`.
///
/// If a dependency is unreachable it reports itself degraded (`503`); if a
/// dependency rejects the input it forwards the `422`.
#[utoipa::path(
    post,
    path = "/",
    request_body = EvalRequest,
    responses(
        (status = 200, body = GeometricSeriesSumResponse),
        (status = 422, description = "a dependency rejected the input (forwarded)"),
        (status = 500, description = "a dependency returned a malformed result"),
        (status = 503, description = "a dependency is unavailable")
    )
)]
pub async fn evaluate(State(deps): State<Deps>, Json(req): Json<EvalRequest>) -> Response {
    let (first, ratio, n) = (req.first, req.ratio, req.n);

    // ratio == 1: all terms equal `first`, so the sum is `first * n`.
    if ratio == 1 {
        let body = match ask(
            &deps.multiply_url,
            &json!({ "a": first, "b": n }),
            "srvcs-multiply",
        )
        .await
        {
            Ok(body) => body,
            Err(resp) => return resp,
        };
        let result = match body.get("result").and_then(Value::as_i64) {
            Some(r) => r,
            None => return malformed("srvcs-multiply"),
        };
        return ok(first, ratio, n, result);
    }

    // 1. rn = ratio^n
    let power_body = match ask(
        &deps.power_url,
        &json!({ "base": ratio, "exp": n }),
        "srvcs-power",
    )
    .await
    {
        Ok(body) => body,
        Err(resp) => return resp,
    };
    let rn = match power_body.get("result").and_then(Value::as_i64) {
        Some(v) => v,
        None => return malformed("srvcs-power"),
    };

    // 2. num = rn - 1
    let sub1_body = match ask(
        &deps.subtract_url,
        &json!({ "a": rn, "b": 1 }),
        "srvcs-subtract",
    )
    .await
    {
        Ok(body) => body,
        Err(resp) => return resp,
    };
    let num = match sub1_body.get("result").and_then(Value::as_i64) {
        Some(v) => v,
        None => return malformed("srvcs-subtract"),
    };

    // 3. num2 = first * num
    let mul_body = match ask(
        &deps.multiply_url,
        &json!({ "a": first, "b": num }),
        "srvcs-multiply",
    )
    .await
    {
        Ok(body) => body,
        Err(resp) => return resp,
    };
    let num2 = match mul_body.get("result").and_then(Value::as_i64) {
        Some(v) => v,
        None => return malformed("srvcs-multiply"),
    };

    // 4. den = ratio - 1
    let sub2_body = match ask(
        &deps.subtract_url,
        &json!({ "a": ratio, "b": 1 }),
        "srvcs-subtract",
    )
    .await
    {
        Ok(body) => body,
        Err(resp) => return resp,
    };
    let den = match sub2_body.get("result").and_then(Value::as_i64) {
        Some(v) => v,
        None => return malformed("srvcs-subtract"),
    };

    // 5. result = num2 / den
    let div_body = match ask(
        &deps.divide_url,
        &json!({ "a": num2, "b": den }),
        "srvcs-divide",
    )
    .await
    {
        Ok(body) => body,
        Err(resp) => return resp,
    };
    let result = match div_body.get("result").and_then(Value::as_i64) {
        Some(v) => v,
        None => return malformed("srvcs-divide"),
    };

    ok(first, ratio, n, result)
}

#[derive(OpenApi)]
#[openapi(
    paths(index, evaluate),
    components(schemas(Info, EvalRequest, GeometricSeriesSumResponse))
)]
pub struct ApiDoc;

/// Serve OpenAPI document
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_documents_routes() {
        let doc = ApiDoc::openapi();
        let root = doc.paths.paths.get("/").expect("path / present");
        assert!(root.get.is_some());
        assert!(root.post.is_some());
    }

    #[tokio::test]
    async fn index_reports_all_dependencies() {
        let Json(info) = index().await;
        assert_eq!(info.service, "srvcs-geometricseriessum");
        assert_eq!(
            info.concern,
            "sequences: sum of first n terms of a geometric sequence"
        );
        assert_eq!(
            info.depends_on,
            vec![
                "srvcs-power",
                "srvcs-multiply",
                "srvcs-subtract",
                "srvcs-divide"
            ]
        );
    }
}
