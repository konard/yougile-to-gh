---
bump: minor
---

### Added
- Authenticate to YouGile with a login and password: the CLI exchanges credentials for an API key via the `AuthKeyController` endpoints (`POST /api-v2/auth/companies` and `POST /api-v2/auth/keys`) when no token is supplied.
- Persist a credential-resolved token to a `.lenv` file (`--lenv-path`, default `.lenv`) for reuse, with `--no-save-token` to opt out.
- Adopt the `lino-arguments` library so CLI options, environment variables, `.lenv`, and `.env` are loaded with a consistent precedence.
- Added `--yougile-login`, `--yougile-password`, and `--yougile-company-id` options (and matching `YOUGILE_LOGIN` / `YOUGILE_PASSWORD` / `YOUGILE_COMPANY_ID` environment variables); a single accessible company is selected automatically.
- Added issue #5 case-study documentation under `docs/case-studies/issue-5/`.
