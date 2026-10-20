create table roles (
    id          uuid primary key default uuid_v7(),
    name        text not null unique,
    description text,
    created_at  timestamptz not null default now()
);

insert into roles (name, description) values
    ('user', 'Default role granted to every signed-up account.'),
    ('admin', 'Full access to the admin panel.');
