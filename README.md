# srvcs-geometricseriessum

## Name

| Field | Value |
| --- | --- |
| Service | `srvcs-geometricseriessum` |
| Slug | `geometricseriessum` |
| Repository | `srvcs/geometricseriessum` |
| Package | `srvcs-geometricseriessum` |
| Kind | `orchestrator` |

## Function

sequences: sum of first n terms of a geometric sequence

## Dependencies

| Dependency | Repository |
| --- | --- |
| `srvcs-power` | [srvcs/power](https://github.com/srvcs/power) |
| `srvcs-multiply` | [srvcs/multiply](https://github.com/srvcs/multiply) |
| `srvcs-subtract` | [srvcs/subtract](https://github.com/srvcs/subtract) |
| `srvcs-divide` | [srvcs/divide](https://github.com/srvcs/divide) |

## API

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/` | Service identity |
| `POST` | `/` | Evaluate the service function |
| `GET` | `/healthz` | Liveness probe |
| `GET` | `/readyz` | Readiness probe |
| `GET` | `/metrics` | Prometheus metrics |
| `GET` | `/openapi.json` | OpenAPI document |

## Inputs

| Name | Type | Required |
| --- | --- | --- |
| `first` | `integer` | yes |
| `ratio` | `integer` | yes |
| `n` | `integer` | yes |

## Outputs

| Name | Type |
| --- | --- |
| `first` | `integer` |
| `ratio` | `integer` |
| `n` | `integer` |
| `result` | `integer` |

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `SRVCS_BIND_ADDR` | `0.0.0.0:8080` | Bind address |
| `SRVCS_ENV` | `development` | Environment label for logs |
| `RUST_LOG` | `info,tower_http=info` | Tracing filter |
| `SRVCS_DIVIDE_URL` | `http://127.0.0.1:8093` | Base URL for srvcs-divide |
| `SRVCS_MULTIPLY_URL` | `http://127.0.0.1:8091` | Base URL for srvcs-multiply |
| `SRVCS_POWER_URL` | `http://127.0.0.1:8090` | Base URL for srvcs-power |
| `SRVCS_SUBTRACT_URL` | `http://127.0.0.1:8092` | Base URL for srvcs-subtract |

## Error Behavior

- `422` means the request could not be evaluated for the documented input shape.
- `503` means a required dependency was unavailable or returned an unexpected response.
- Dependency validation errors are forwarded when this service delegates validation.

## Local Checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

See the [srvcs service standard](https://github.com/srvcs/platform/blob/main/STANDARD.md) for the full operational contract.

## Metadata

Machine-readable service metadata lives in `srvcs.yaml`. Keep it aligned with this README when the service contract changes.
