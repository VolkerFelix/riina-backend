-- Add profile picture URL columns to game_summaries table for MVP and LVP

BEGIN;

-- Add profile picture URL columns for MVP and LVP
ALTER TABLE game_summaries 
ADD COLUMN mvp_profile_picture_url VARCHAR(500),
ADD COLUMN lvp_profile_picture_url VARCHAR(500);

-- Add comments to explain the new columns
COMMENT ON COLUMN game_summaries.mvp_profile_picture_url IS 'Profile picture URL for the MVP (Most Valuable Player)';
COMMENT ON COLUMN game_summaries.lvp_profile_picture_url IS 'Profile picture URL for the LVP (Least Valuable Player)';

COMMIT;
