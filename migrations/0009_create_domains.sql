create table domains (
    id               bigserial primary key,
    certification_id bigint not null references certifications(id) on delete cascade,
    name             text not null,
    sort_order       integer not null,
    unique (certification_id, name)
);
