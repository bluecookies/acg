SELECT s.diff, AVG(p.guess_rate), COUNT(p.guess_rate), SUM(p.times_played)
FROM song_plays p
LEFT JOIN (
    SELECT id, width_bucket(difficulty, 0, 100, $1) AS diff
    FROM amq_songs
) s
ON p.amq_song_id = s.id
WHERE player = 1
GROUP BY s.diff
HAVING s.diff IS NOT NULL
ORDER BY s.diff DESC;