insert into roles (name, description) values
    ('editor', 'Can create and edit questions, but not manage users or roles.');

insert into role_permissions (role_id, permission_id)
select r.id, p.id
from roles r
join permissions p on p.key in ('admin.access', 'certifications.write')
where r.name = 'editor';
