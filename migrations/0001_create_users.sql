create table users (
    id            uuid primary key default gen_random_uuid(),
    name          text not null,
    email         citext not null unique,
    password_hash text not null,
    created_at    timestamptz not null default now(),
    updated_at    timestamptz not null default now()
);
