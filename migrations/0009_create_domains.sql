create table domains (
    id               uuid primary key default uuid_v7(),
    certification_id uuid not null references certifications(id) on delete cascade,
    name             text not null,
    sort_order       integer not null,
    unique (certification_id, name)
);
