UPDATE song_plays
SET guess_rate = LENGTH(REPLACE(correct_guesses::TEXT, '0', ''))::REAL/NULLIF(LENGTH(correct_guesses), 0)
WHERE amq_song_id = $1
AND player = $2
