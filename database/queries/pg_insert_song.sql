INSERT INTO amq_songs (
    songname, artist,
    anime_id,
    type,
    mp3, video,
    video_length,
    difficulty,
    created_date, modified_date
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
RETURNING id
