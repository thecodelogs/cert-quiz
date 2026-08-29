-- One row per paste on the Import page — what got accepted/rejected and
-- why, so "Recent imports" has something real to show.
create table imports (
    id                bigserial primary key,
    certification_id  bigint not null references certifications(id),
    format            text not null check (format in ('json', 'csv')),
    rows_accepted     integer not null default 0,
    rows_rejected     integer not null default 0,
    rejected_reason   text,
    imported_by       uuid references users(id) on delete set null,
    created_at        timestamptz not null default now()
);

create index on imports (certification_id, created_at);
