# srvcs-geometricseriessum

The geometric-series-sum orchestrator of the srvcs.cloud distributed standard
library.

Its single concern: **sequences: sum of first n terms of a geometric
sequence.** It owns the *control flow* — composing four primitives — but does no
arithmetic of its own. It asks
[`srvcs-power`](https://github.com/srvcs/power),
[`srvcs-multiply`](https://github.com/srvcs/multiply),
[`srvcs-subtract`](https://github.com/srvcs/subtract) and
[`srvcs-divide`](https://github.com/srvcs/divide) to assemble the result.

```
geometricseriessum(first, ratio, n):
    if ratio == 1:
        return multiply(first, n)        # every term equals `first`
    rn   = power(ratio, n)               # ratio^n
    num  = subtract(rn, 1)               # ratio^n - 1
    num2 = multiply(first, num)          # first * (ratio^n - 1)
    den  = subtract(ratio, 1)            # ratio - 1
    return divide(num2, den)             # first * (ratio^n - 1) / (ratio - 1)
```

For example, `geometricseriessum(first=1, ratio=2, n=4) == 15` (1 + 2 + 4 + 8).

Validation is not handled here. This service never calls `srvcs-isnumber`
directly; instead its dependencies validate their own operands, and any `422`
they raise is forwarded verbatim.

## API

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/` | Service identity, concern, and dependency list |
| `POST` | `/` | Compute the sum of the first `n` terms of a geometric sequence |
| `GET` | `/healthz` `/readyz` `/metrics` `/openapi.json` | srvcs service standard surface |

```sh
curl -s -X POST localhost:8080/ -H 'content-type: application/json' \
  -d '{"first": 1, "ratio": 2, "n": 4}'
# {"first":1,"ratio":2,"n":4,"result":15}
```

Responses:

- `200 {"first": first, "ratio": ratio, "n": n, "result": <int>}` — evaluated;
  `result` is an integer.
- `422` — a dependency rejected the input, forwarded verbatim.
- `500` — a reachable dependency returned a `200` without an integer `result`
  (a contract violation).
- `503` — a dependency is unavailable.

## Dependencies

- [`srvcs-power`](https://github.com/srvcs/power)
- [`srvcs-multiply`](https://github.com/srvcs/multiply)
- [`srvcs-subtract`](https://github.com/srvcs/subtract)
- [`srvcs-divide`](https://github.com/srvcs/divide)

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `SRVCS_BIND_ADDR` | `0.0.0.0:8080` | Bind address |
| `SRVCS_POWER_URL` | `http://127.0.0.1:8090` | Base URL of `srvcs-power` |
| `SRVCS_MULTIPLY_URL` | `http://127.0.0.1:8091` | Base URL of `srvcs-multiply` |
| `SRVCS_SUBTRACT_URL` | `http://127.0.0.1:8092` | Base URL of `srvcs-subtract` |
| `SRVCS_DIVIDE_URL` | `http://127.0.0.1:8093` | Base URL of `srvcs-divide` |
| `SRVCS_ENV` | `development` | Environment label for logs |
| `RUST_LOG` | `info,tower_http=info` | Tracing filter |

## Local checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Orchestration tests stand up *computing* mock `srvcs-power`, `srvcs-multiply`,
`srvcs-subtract` and `srvcs-divide` services in-process — they read the request
body and return the real `base^exp` / `a * b` / `a - b` / `a / b`, so the
closed-form composition is genuinely exercised against the asserted cases. See
[`srvcs/platform`](https://github.com/srvcs/platform) for the shared standard.

> Note: the `cargoHash` in `flake.nix` is inherited from the template and must be
> refreshed with a `nix build` before the Nix gates pass.
