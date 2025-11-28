-- Add vt_off_threshold column to user_health_profiles
ALTER TABLE user_health_profiles
ADD COLUMN vt_off_threshold INTEGER;

-- Calculate and populate vt_off_threshold for existing users
UPDATE user_health_profiles
SET vt_off_threshold = resting_heart_rate + ((max_heart_rate - resting_heart_rate) * 0.20)::INTEGER
WHERE max_heart_rate IS NOT NULL AND resting_heart_rate IS NOT NULL;
