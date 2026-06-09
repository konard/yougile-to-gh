# Issue 5 Case Study: YouGile Credential Auth and `.lenv` Token Persistence

Source issue: <https://github.com/konard/yougile-to-gh/issues/5>

Pull request: <https://github.com/konard/yougile-to-gh/pull/6>

## Issue Data

Issue title: "Add Yougile auto auth using user credentials, and auto save token to .lenv"

Labels: `documentation`, `enhancement`

Created: 2026-06-09T14:17:09Z

Comments at implementation start: none (`raw-data/issue-5-comments.json` is `[]`).

### Verbatim issue body

> For .lenv and CLI options we use <https://github.com/link-foundation/lino-arguments>
>
> For token:
> <https://ru.yougile.com/api-v2#/operations/getCompanies>
> <https://ru.yougile.com/api-v2#/operations/AuthKeyController_create>
>
> We need to collect data related about the issue to this repository, make sure we compile that data to `./docs/case-studies/issue-{id}` folder, and use it to do deep case study analysis (also make sure to search online for additional facts and data), list of each and all requirements from the issue, and propose possible solutions and solution plans for each requirement (we should also check known existing components/libraries, that solve similar problem or can help in solutions).
>
> Please plan and execute everything in this single pull request, you have unlimited time and context, as context auto-compacts and you can continue indefinitely, until it is each and every requirement fully addressed, and everything is totally done.

Raw data captured for this case study lives in [`raw-data/`](./raw-data):

- `issue-5.json` — the issue payload.
- `issue-5-comments.json` — issue comments (empty).
- `pr-6.json` — the existing pull request payload.
- `yougile-openapi-auth.json` — the auth subset of the official YouGile OpenAPI document (`https://ru.yougile.com/api-json`), retained as the authoritative API reference.

## Requirements

The issue contains two layers of requirements: the feature itself, and the process/analysis the author asks for.

### Feature requirements

1. **R1 — Credential authentication.** Authenticate to YouGile using the user's credentials (login/email + password) instead of requiring a pre-existing API token.
2. **R2 — Token creation via the documented endpoints.** Obtain an API key through the operations named in the issue:
   - `getCompanies` (`POST /api-v2/auth/companies`) to discover which company the credentials can access.
   - `AuthKeyController_create` (`POST /api-v2/auth/keys`) to create the API key.
3. **R3 — Auto-save the token to `.lenv`.** Persist the resolved token to a `.lenv` file so subsequent runs reuse it instead of re-authenticating.
4. **R4 — Use `lino-arguments`.** Use the `lino-arguments` library for `.lenv` and CLI option handling.

### Process requirements

5. **R5 — Collect issue data** into `./docs/case-studies/issue-5/` (this folder).
6. **R6 — Deep case-study analysis** including additional online research.
7. **R7 — Enumerate every requirement** from the issue (this section).
8. **R8 — Propose solutions/solution plans** per requirement, surveying existing components/libraries that solve a similar problem.
9. **R9 — Single pull request.** Plan and execute everything in PR #6 until every requirement is fully addressed.

## Online Research

### YouGile authentication API (authoritative)

The interactive docs page at `https://ru.yougile.com/api-v2` is a JavaScript-rendered Stoplight UI, but the backing OpenAPI document is served as JSON at `https://ru.yougile.com/api-json` and is publicly readable. The auth subset is saved in [`raw-data/yougile-openapi-auth.json`](./raw-data/yougile-openapi-auth.json). Key facts extracted from it:

- **`POST /api-v2/auth/companies`** (`operationId: getCompanies`)
  - Request body `CredentialsWithNameDto`: `login` (required), `password` (required), `name` (optional company-name filter).
  - Response `200` → `CompanyListDto`: `{ "paging": Paging, "content": CompanyDto[] }`.
  - `CompanyDto`: `id` (string, required), **`title`** (string — the company name), `timestamp` (number, required), `deleted` (bool), `apiData` (object). Note there is no `name` or `isAdmin` field in the official DTO; the name lives in `title`.
  - Error responses: `401`, `403`, `429` (the API is rate limited).

- **`POST /api-v2/auth/keys`** (`operationId: AuthKeyController_create`)
  - Request body `CredentialsWithCompanyDto`: `login`, `password`, `companyId` (all required).
  - Response `201` → `AuthKeyDto`: `{ "key": string }` (example `"H6HngIA816fpIhY7tBvWx/it3YbVvEt/33Sk8afA39MCR9a"`).
  - Error responses: `400`, `401`, `403`, `429`.

This confirms the two-step flow the issue links to: list companies with credentials, then create a key scoped to a chosen `companyId`. The base URL follows the existing `https://{host}/api-v2/{resource}` convention already used by the converter's `YougileClient` (see issue #1's case study), and the resulting key is used as a `Authorization: Bearer <key>` token for subsequent task fetches. Per YouGile's admin guide the API is rate limited (the `429` responses above), so credential exchange should happen once and the token should be cached — which is exactly what R3 asks for.

> **Research-driven fix:** because the official `CompanyDto` exposes the company name as `title` (not `name`), the `YougileCompany` model deserializes the name from `title` with a `name` alias for community-client compatibility. Without this, the human-readable company name in the "multiple companies" error message would always be blank against the real API.

### `lino-arguments` library

`lino-arguments` (<https://github.com/link-foundation/lino-arguments>, crate `lino-arguments` v0.3) is described by its authors as "a unified configuration library combining environment variables and CLI arguments with a clear priority chain." The documented precedence is:

1. CLI arguments (highest priority)
2. Environment variables (case-insensitive lookup)
3. Configuration file (the `.lenv`/`.env` files)
4. Default values (fallback)

In Rust it is a drop-in replacement for `clap`'s derive API: it re-exports `Parser`, `Args`, `Subcommand`, `ValueEnum`, `arg`, and `command`, and additionally re-exports `lino-env`'s `LinoEnv`/`read_lino_env`/`write_lino_env`. On startup it auto-loads `.lenv` and then `.env` into the process environment (non-overriding), and also exposes an explicit `init()` for doing so deterministically before argument parsing. The `.lenv` file format is `KEY: value` (colon-space), handled by the `lino-env` crate. This makes it a natural fit for both R3 (write the token to `.lenv`) and R4 (use the library for `.lenv` + CLI options).

## Existing Components / Libraries Surveyed

| Need | Existing option | Decision |
| --- | --- | --- |
| CLI + env + dotfile config with precedence | `lino-arguments` (re-exports clap + lino-env) | **Adopted** (mandated by R4); replaced the bare `clap` dependency. |
| `.env`-style persistence | `dotenvy`, `config`, `figment` | Not needed directly — `lino-arguments`/`lino-env` already cover `.lenv`/`.env` loading and writing. |
| YouGile API access | Existing in-repo `YougileClient` + `HttpClient` trait (issue #1) | **Reused.** The new `YougileAuth` shares the same `HttpClient` abstraction and `normalize_api_base_url` helper for testability. |
| HTTP client | In-repo `UreqHttpClient` (wraps `ureq`) | **Reused** — no new HTTP dependency. |
| Community YouGile clients (e.g. Go `yougilego`, Python wrappers) | reference only | Used to cross-check response envelope shapes; the official OpenAPI is authoritative. |

## Solution Plans Per Requirement

- **R1 / R2 (credential auth + token creation).** Add a credential-only `YougileAuth<C: HttpClient>` client with `list_companies`, `create_api_key`, and a `resolve_token` orchestration method. `resolve_token` uses an explicit `--yougile-company-id` when given; otherwise it lists companies and auto-selects when exactly one is accessible, returning a descriptive error for the zero- and many-company cases. Implemented in `src/auth.rs`.
- **R3 (auto-save to `.lenv`).** After a token is resolved from credentials, write `YOUGILE_TOKEN` into the `.lenv` file via `LinoEnv` (read-modify-write, preserving existing entries). Path is configurable with `--lenv-path` (default `.lenv`); `--no-save-token` opts out. A token supplied directly (including one loaded back from `.lenv`) short-circuits the credential flow. Implemented in `src/main.rs` (`resolve_yougile_token`, `save_token_to_lenv`).
- **R4 (use `lino-arguments`).** Replace the direct `clap` dependency with `lino-arguments`; import `Parser`/`ValueEnum`/`LinoEnv` from it and call `lino_arguments::init()` before `Args::parse()` so a previously persisted token is reused. Implemented in `Cargo.toml` and `src/main.rs`.
- **R5–R8 (data, analysis, requirements, solutions).** This document plus the captured `raw-data/`.
- **R9 (single PR).** All work lands in PR #6 on branch `issue-5-7c2e7cba7d85`.

## Alternatives Considered

1. **Prompt interactively for the company when several exist** vs. **error out listing the choices.** Chosen the latter: interactive prompts are awkward in CI/non-TTY usage, and a clear error naming each `id (title)` lets the user re-run with `--yougile-company-id`.
2. **Store the whole credential set in `.lenv`** vs. **store only the resolved token.** Chosen to persist only the token, so the long-lived secret on disk is the API key (revocable in YouGile) rather than the account password.
3. **Add a new config crate (`figment`/`config`)** vs. **use `lino-arguments`.** R4 mandates `lino-arguments`, which also removes the need for a separate dotfile loader.
4. **Always re-authenticate** vs. **cache the token.** Caching (R3) avoids hitting the rate-limited (`429`) auth endpoints on every run.

## Implemented Plan

- `Cargo.toml`: depend on `lino-arguments` (replacing the bare `clap` entry for CLI parsing).
- `src/auth.rs`: new `YougileAuth`, `YougileCompany` (name from `title` with `name` alias), and `ResolvedToken`, built on the existing `HttpClient` trait.
- `src/error.rs`: new `YougileNoCompanies`, `YougileMultipleCompanies { companies }`, and `YougileMissingApiKey` variants.
- `src/yougile.rs`: expose `normalize_api_base_url` as `pub(crate)` for reuse by `auth`.
- `src/main.rs`: new `--yougile-login`, `--yougile-password`, `--yougile-company-id`, `--lenv-path`, and `--no-save-token` options; `resolve_yougile_token`/`save_token_to_lenv`; `lino_arguments::init()` before parsing.
- `README.md` + `.gitignore`: document the credential flow and ignore the secret-bearing `.lenv`.
- `changelog.d/`: a `bump: minor` fragment describing the feature.

## Verification Strategy

- **Unit tests** (`tests/unit/auth.rs`): a `FakeHttp` mock (keyed by URL suffix, recording requests) covers key creation, single-company auto-selection, explicit-company short-circuit, the zero/many-company errors, the missing-key error, the official `title`-based `CompanyDto`, the community `companies` envelope, and that requests are `POST`ed to the `/api-v2/auth/*` endpoints with the expected payload.
- **Integration test** (`tests/integration/cli.rs`): a mock YouGile server serves the full handshake (`companies` → `keys` → task → messages); the compiled CLI runs with `--yougile-login/--yougile-password` and `--dry-run`, and the test asserts the resolved token is used as a bearer credential, the token is written to `.lenv` in `KEY: value` form, and neither the token nor the password leak to stdout/stderr.
- **Local checks:** `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `rust-script scripts/check-file-size.rs`.
