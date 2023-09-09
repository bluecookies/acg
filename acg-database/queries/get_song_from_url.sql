SELECT songname, artist
FROM amq_songs
WHERE mp3 = $1 OR video = $2
ORDER BY modified_date DESC
LIMIT 1;
