INSERT INTO song_anime_tags (
    amq_anime_id,
    tag_type, tag,
    modified_date
)
VALUES ($1, $2, $3, $4)
ON CONFLICT (amq_anime_id, tag_type, tag) DO
UPDATE SET
    modified_date = excluded.modified_date
