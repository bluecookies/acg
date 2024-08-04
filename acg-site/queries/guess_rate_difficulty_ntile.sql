SELECT
  AVG(guess_rate),
  MIN(difficulty),
  MAX(difficulty),
  COUNT(*),
  kind,
  bucket
FROM
  (
    SELECT
      p.guess_rate,
      s.difficulty,
      s.kind,
      ntile($1) OVER (
        PARTITION BY s.kind
        ORDER BY s.difficulty
      ) AS bucket
    FROM
      (
        SELECT *, type AS kind
        FROM amq_songs
        UNION
        SELECT *, 'All' AS kind
        FROM amq_songs
      ) s
      LEFT JOIN song_plays p ON s.id = p.amq_song_id
    WHERE
      guess_rate IS NOT NULL
      AND p.player = 1
    ORDER BY s.difficulty
  ) d
GROUP BY bucket, kind
ORDER BY bucket;