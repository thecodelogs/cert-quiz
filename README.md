# Drill — Rust / Axum / Tera

Same design and behaviour as the Express version, server-rendered with Tera.

```
rust-app/
  Cargo.toml
  src/main.rs           routes, form parsing, view models
  src/model.rs          Bank/Cert/Question types, scoring
  data/questions.json   5 certifications, 60 questions
  templates/
    home.html           cert index
    question.html       one question per page
    results.html        score, domain breakdown, review
    partials/head.html  doctype, fonts, Tailwind CDN
    partials/foot.html  client script + close
  static/
    tailwind.config.js  theme tokens
    app.js              elapsed timer
```

## Run

```
cd rust-app
cargo run          # http://localhost:3000
```

Templates and `data/questions.json` are read from the working directory, so run from the crate root (Tera globs `templates/**/*.html` at startup; restart to pick up template edits, or swap in `Tera::full_reload` during development).

## Routes

| Method | Path | Renders |
| --- | --- | --- |
| GET | `/?provider=AWS\|GCP\|K8s\|All` | `home.html` |
| GET | `/quiz/:cert_id` | `question.html` at question 1 |
| POST | `/quiz/:cert_id` | next / previous question, or `results.html` |

The quiz is one form per question. Answers so far ride in a hidden `answers` field as JSON and are parsed with `form_urlencoded` (repeated `choice` keys give multi-select), so no session store is needed yet.

## Where to make it dynamic

- `Bank::load` reads the JSON once at startup — replace with a repository over your database (sqlx/SeaORM); `Bank::cert` and `Cert::set` are the only lookups the handlers use.
- `Cert::set` returns the first `SET_LENGTH` (20) questions in order; randomise here.
- `model::score` is pure — reuse it when you persist attempts.
- `home` inserts `history: []`; feed it `{ name, when, time, pct }` rows once attempts are stored, and replace `best_label` per cert.
- To move answers server-side, put `Answers` in a session (e.g. `tower-sessions`) and drop the hidden fields.

## Production Tailwind

The CDN compiles in the browser. For production, build a stylesheet with the Tailwind CLI (`content: ['./templates/**/*.html']`), write it to `static/`, and replace the two `<script>` tags in `partials/head.html` with a `<link>`.
