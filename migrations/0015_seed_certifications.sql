-- Real content, generated once from data/questions.json before that file
-- was removed; certifications, domains, questions and options below are
-- transcribed verbatim from it, not synthetic dummy data.

insert into certifications (slug, provider_id, code, name, difficulty, blurb, pass_mark)
select v.slug, p.id, v.code, v.name, v.difficulty, v.blurb, v.pass_mark
from (values
    ('saa', 'AWS', 'SAA-C03', 'Solutions Architect – Associate', 'Associate', 'Resilient, cost-aware architecture across compute, storage, networking and data.', 72),
    ('dva', 'AWS', 'DVA-C02', 'Developer – Associate', 'Associate', 'Building, deploying and debugging serverless and container applications on AWS.', 72),
    ('ace', 'GCP', 'GCP-ACE', 'Associate Cloud Engineer', 'Associate', 'Deploying and operating solutions, plus day-to-day access and billing management.', 72),
    ('pca', 'GCP', 'GCP-PCA', 'Professional Cloud Architect', 'Professional', 'Designing, provisioning and evolving solutions with business and compliance constraints.', 72),
    ('cka', 'K8s', 'CKA', 'Certified Kubernetes Administrator', 'Professional', 'Cluster architecture, workloads, networking, storage and troubleshooting.', 72)
) as v(slug, provider_name, code, name, difficulty, blurb, pass_mark)
join providers p on p.name = v.provider_name;
