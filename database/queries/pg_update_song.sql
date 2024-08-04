UPDATE amq_songs
SET
    songname = $1,
    artist = $2,
    anime_id = $3,
    type = $4,
    typenum = $5,
    mp3 = COALESCE($6, mp3),
    video = COALESCE($7, video),
    video_length = COALESCE($8, video_length),
    difficulty = $9,
    modified_date = $10
WHERE anime_id = $3
AND (
    (video = $7 AND (mp3 = $6 OR mp3 IS NULL))
    OR (songname = $1 AND artist = $2 AND type = $4 AND typenum = $5)
)
RETURNING id, (SELECT difficulty FROM amq_songs s WHERE s.id = amq_songs.id)
