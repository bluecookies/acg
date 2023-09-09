SELECT
  a.vintage,
  s.kind,
  AVG(guess_rate),
  COUNT(guess_rate),
  SUM(p.times_played)
FROM song_plays p
LEFT JOIN (
  SELECT *, SPLIT_PART(type, ' ', 1) AS kind FROM amq_songs
  UNION
  SELECT *, 'All' FROM amq_songs
) s
ON p.amq_song_id = s.id
LEFT JOIN amq_anime a
ON s.anime_id = a.ann_id
WHERE player = 1
GROUP BY a.vintage, s.kind;