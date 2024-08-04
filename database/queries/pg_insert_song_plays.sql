INSERT INTO song_plays (
    amq_song_id,
    player,
    times_played,
    correct_guesses,
    guess_rate,
    last_played
)
-- song_id, name, correct, date
VALUES (
    $1,
    (SELECT id FROM users WHERE name = $2),
    1,
    COALESCE($3::INT::BIT, b''),
    $3::INT,
    $4
)
ON CONFLICT (amq_song_id, player) DO
UPDATE SET
    times_played = song_plays.times_played + 1,
    correct_guesses = SUBSTRING(excluded.correct_guesses || song_plays.correct_guesses, 1, 16),
    last_played = $4
RETURNING 
    player, 
    times_played, 
    correct_guesses, 
    (SELECT last_played FROM song_plays p 
    WHERE p.amq_song_id = song_plays.amq_song_id
    AND p.player = song_plays.player)
