INSERT INTO amq_anime_names (ann_id, name, modified_date)
VALUES ($1, $2, $3)
ON CONFLICT (ann_id, name) DO
UPDATE SET modified_date = excluded.modified_date
