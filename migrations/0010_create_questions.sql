create table questions (
    id               uuid primary key default uuid_v7(),
    certification_id uuid not null references certifications(id) on delete cascade,
    domain_id        uuid not null references domains(id),
    body             text not null,
    explain          text not null,
    sort_order       integer not null,
    created_at       timestamptz not null default now(),
    updated_at       timestamptz not null default now()
);

create index on questions (certification_id, sort_order);
