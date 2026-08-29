create table user_roles (
    user_id    uuid     not null references users(id) on delete cascade,
    role_id    smallint not null references roles(id) on delete cascade,
    granted_at timestamptz not null default now(),
    primary key (user_id, role_id)
);

create index on user_roles (role_id);
