create table role_permissions (
    role_id       smallint not null references roles(id) on delete cascade,
    permission_id bigint   not null references permissions(id) on delete cascade,
    primary key (role_id, permission_id)
);

create index on role_permissions (permission_id);

-- admin starts with every permission that exists today
insert into role_permissions (role_id, permission_id)
select r.id, p.id
from roles r
cross join permissions p
where r.name = 'admin';
