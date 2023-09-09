WITH escaped AS (
  SELECT REPLACE(REPLACE(REPLACE($1, '\', '\\'), '%', '\%'), '_', '\_') AS escaped
),
search AS (
  -- exact
  SELECT
  CASE
   WHEN $2 THEN escaped
   ELSE '%'||escaped||'%'
  END search,
  CASE
   WHEN $2 THEN escaped||'.%'
   ELSE escaped||'%'
  END search_link
  FROM escaped
)
SELECT DISTINCT ON(s.id) s.id, songname, artist, romaji, difficulty
FROM amq_songs s
LEFT JOIN amq_anime a
ON s.anime_id = a.ann_id
LEFT JOIN amq_anime_names an
ON a.ann_id = an.ann_id
CROSS JOIN search
WHERE
  s.songname ILIKE search
OR
  s.artist ILIKE search
OR
  an.name ILIKE search
OR
  s.mp3 LIKE search_link
OR
  s.video LIKE search_link
LIMIT 1000;