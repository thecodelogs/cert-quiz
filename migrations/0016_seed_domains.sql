insert into domains (certification_id, name, sort_order)
select c.id, v.name, v.sort_order
from (values
    ('saa', 'Secure architectures', 0),
    ('saa', 'Resilient architectures', 1),
    ('saa', 'High-performing architectures', 2),
    ('saa', 'Cost-optimized architectures', 3),
    ('dva', 'Development with AWS services', 0),
    ('dva', 'Security', 1),
    ('dva', 'Deployment', 2),
    ('dva', 'Troubleshooting & optimization', 3),
    ('ace', 'Setting up a cloud environment', 0),
    ('ace', 'Planning & configuring', 1),
    ('ace', 'Deploying & implementing', 2),
    ('ace', 'Ensuring successful operation', 3),
    ('ace', 'Access & security', 4),
    ('pca', 'Solution design', 0),
    ('pca', 'Security & compliance', 1),
    ('pca', 'Reliability & operations', 2),
    ('pca', 'Migration & modernisation', 3),
    ('cka', 'Cluster architecture & installation', 0),
    ('cka', 'Workloads & scheduling', 1),
    ('cka', 'Services & networking', 2),
    ('cka', 'Storage', 3),
    ('cka', 'Troubleshooting', 4)
) as v(slug, name, sort_order)
join certifications c on c.slug = v.slug;
