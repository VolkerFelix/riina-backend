-- Rename home_score and away_score to home_score_final and away_score_final
-- to clarify these are the final results transferred from live games

ALTER TABLE league_games RENAME COLUMN home_score TO home_score_final;
ALTER TABLE league_games RENAME COLUMN away_score TO away_score_final;