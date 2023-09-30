SELECT 
  songname,
  artist,
  url
FROM karaoke_sa.songs
ORDER BY RANDOM() 
LIMIT 1;
