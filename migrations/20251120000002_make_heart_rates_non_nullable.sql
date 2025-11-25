-- Make resting_heart_rate and max_heart_rate non-nullable with default values
-- Set default values for existing NULL records first
UPDATE user_health_profiles
SET resting_heart_rate = 65
WHERE resting_heart_rate IS NULL;

UPDATE user_health_profiles
SET max_heart_rate = 190
WHERE max_heart_rate IS NULL;

-- Now alter columns to be non-nullable with defaults
ALTER TABLE user_health_profiles
ALTER COLUMN resting_heart_rate SET DEFAULT 65,
ALTER COLUMN resting_heart_rate SET NOT NULL;

ALTER TABLE user_health_profiles
ALTER COLUMN max_heart_rate SET DEFAULT 190,
ALTER COLUMN max_heart_rate SET NOT NULL;

-- Update the constraint to reflect non-nullable values
ALTER TABLE user_health_profiles
DROP CONSTRAINT IF EXISTS reasonable_max_heart_rate;

ALTER TABLE user_health_profiles
ADD CONSTRAINT reasonable_max_heart_rate CHECK (
    max_heart_rate >= 100 AND max_heart_rate <= 250
);

ALTER TABLE user_health_profiles
DROP CONSTRAINT IF EXISTS reasonable_heart_rates;

ALTER TABLE user_health_profiles
ADD CONSTRAINT reasonable_heart_rates CHECK (
    resting_heart_rate >= 10 AND resting_heart_rate <= 250
);
