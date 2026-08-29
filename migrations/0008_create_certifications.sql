create table certifications (
    id          bigserial primary key,
    slug        text not null unique,
    provider_id bigint not null references providers(id),
    code        text not null,
    name        text not null unique,
    difficulty  text not null,
    blurb       text not null,
    pass_mark   integer not null,
    created_at  timestamptz not null default now(),
    updated_at  timestamptz not null default now()
);
