-- Remove unused is_active column from league_seasons table
-- This column was causing UTF-8 decoding issues because it exists in the database
-- but not in the Rust model definition
ALTER TABLE league_seasons DROP COLUMN IF EXISTS is_active;