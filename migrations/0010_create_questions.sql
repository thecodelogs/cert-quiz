create table questions (
    id               bigserial primary key,
    certification_id bigint not null references certifications(id) on delete cascade,
    domain_id        bigint not null references domains(id),
    body             text not null,
    explain          text not null,
    sort_order       integer not null,
    created_at       timestamptz not null default now(),
    updated_at       timestamptz not null default now()
);

create index on questions (certification_id, sort_order);
