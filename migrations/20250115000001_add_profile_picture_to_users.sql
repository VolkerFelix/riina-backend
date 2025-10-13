-- Add profile picture URL to users table
ALTER TABLE users 
ADD COLUMN profile_picture_url VARCHAR(500);

-- Create index for profile picture queries
CREATE INDEX IF NOT EXISTS idx_users_profile_picture ON users(profile_picture_url) WHERE profile_picture_url IS NOT NULL;
