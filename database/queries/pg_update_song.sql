UPDATE amq_songs
SET
    songname = $1,
    artist = $2,
    anime_id = $3,
    type = $4,
    mp3 = COALESCE($5, mp3),
    video = COALESCE($6, video),
    video_length = COALESCE($7, video_length),
    difficulty = $8,
    modified_date = $9
WHERE anime_id = $3
AND (
    (video = $6 AND (mp3 = $5 OR mp3 IS NULL))
    OR (songname = $1 AND artist = $2 AND type = $4)
)
RETURNING id
