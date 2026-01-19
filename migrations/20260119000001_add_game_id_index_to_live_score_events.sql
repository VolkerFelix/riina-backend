-- Add index on game_id for live_score_events to improve player scores query performance
-- This index was missing after the consolidation migration that changed live_game_id to game_id

CREATE INDEX IF NOT EXISTS idx_live_score_events_game_id ON live_score_events(game_id);
