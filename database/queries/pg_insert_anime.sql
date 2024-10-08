INSERT INTO amq_anime (
    ann_id,
    romaji, english,
    mal_id, anilist_id, kitsu_id,
    type, score, vintage,
    created_date, modified_date
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
ON CONFLICT (ann_id) DO
UPDATE SET
    romaji = $2,
    english = $3,
    mal_id = $4,
    anilist_id = $5,
    kitsu_id = $6,
    type = $7,
    score = $8,
    vintage = $9,
    modified_date = $10
