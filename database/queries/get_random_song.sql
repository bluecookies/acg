SELECT 
  songname,
  artist,
  COALESCE(mp3, video)
FROM amq_songs
ORDER BY RANDOM() 
LIMIT 1;
