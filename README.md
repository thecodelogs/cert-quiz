# Drill — Rust / Axum / Tera / Postgres

Practice exams for cloud certifications (AWS, Google Cloud, Kubernetes).
Learners pick a track, answer a 20-question set one page at a time, and get a
score with a per-domain breakdown and answer review. Admins manage the
question bank at `/admin`.

Server-rendered with Axum + Tera, data in PostgreSQL via sqlx.

```
src/main.rs        startup, learner routes (home, quiz, results)
src/model.rs       Cert/Question types, scoring (pure)
src/db.rs          all SQL queries
src/admin.rs       /admin routes and admin auth gate
src/auth.rs        Argon2id password hash/verify
migrations/        schema + seed data, run automatically at startup
templates/         Tera templates (learner pages + admin/)
static/            app.js (quiz clock, auth popup), tailwind.config.js
```

## Run

```sh
docker compose up -d db      # Postgres 16 (user/pass/db = drill)
cp .env.example .env         # sets DATABASE_URL
cargo run                    # http://localhost:3000
```

Or run the app and database together: `docker compose up --build`.
Set `PORT` to listen somewhere other than 3000.

Run from the crate root. `templates/` and `static/` are read from the working
directory at runtime, and templates are loaded only at startup, so restart
after editing them. Migrations are embedded at compile time, so rebuild after
adding one.

Dev logins (seeded):

| email | password | access |
| --- | --- | --- |
| `admin@drill.test` | `admin12345` | admin panel |
| `demo@drill.test` | `demo12345` | learner only |

Hash a new password: `cargo run --example hash_password -- <password>`.

## Routes

| Method | Path | What |
| --- | --- | --- |
| GET | `/?provider=All\|AWS\|GCP\|K8s` | track list |
| GET / POST | `/login`, `/signup`, `/logout` | learner accounts (log in and sign up on one page) |
| GET / POST | `/quiz/:slug` | one question per page, then results. Requires login; guests go to `/login` |
| GET | `/admin` | dashboard (needs the `admin` role) |
| | `/admin/questions`, `/flagged`, `/import`, `/users` | question bank, learner reports, bulk import, user management |

## Deploy

Every push to `main` runs `.github/workflows/deploy.yml`. It builds the release
binary and copies it to `/opt/drill` on the server over SSH, together with
`templates/` and `static/`. It then restarts the `drill` systemd service. The
server needs `DATABASE_URL` set.

## More

- [`PROJECT.md`](PROJECT.md): how the code works, conventions, and known gaps / TODO.
- [`db.md`](db.md): database schema and the reasoning behind it.
