create table providers (
    id    uuid primary key default uuid_v7(),
    name  text not null unique,
    label text not null
);

insert into providers (name, label) values
    ('AWS', 'Amazon Web Services'),
    ('GCP', 'Google Cloud'),
    ('K8s', 'Kubernetes / CNCF');
