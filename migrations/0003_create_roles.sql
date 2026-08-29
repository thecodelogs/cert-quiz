create table roles (
    id          smallserial primary key,
    name        text not null unique,
    description text,
    created_at  timestamptz not null default now()
);

insert into roles (name, description) values
    ('user', 'Default role granted to every signed-up account.'),
    ('admin', 'Full access to the admin panel.');
