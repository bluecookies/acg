SELECT key, value FROM
(
  SELECT s.id, 'id' AS key, anime_id::text AS value
  FROM amq_songs s
UNION
  SELECT s.id, 'mp3', mp3
  FROM amq_songs s
UNION
  SELECT s.id, 'video', video
  FROM amq_songs s
UNION
  SELECT s.id, 'name', name
  FROM amq_songs s
  LEFT JOIN amq_anime_names an
  ON s.anime_id = an.ann_id
) s
WHERE s.id = $1;