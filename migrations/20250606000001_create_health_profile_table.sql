-- Create user health profiles table for baseline physiological data
CREATE TABLE IF NOT EXISTS user_health_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    resting_heart_rate INTEGER,
    max_heart_rate INTEGER,
    age INTEGER,
    weight REAL,
    height REAL,
    gender VARCHAR(10),
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_user_profile UNIQUE(user_id),
    CONSTRAINT reasonable_heart_rates CHECK (
        resting_heart_rate IS NULL OR (resting_heart_rate >= 10 AND resting_heart_rate <= 250)
        AND max_heart_rate IS NULL OR (max_heart_rate >= 10 AND max_heart_rate <= 250)
        AND (resting_heart_rate IS NULL OR max_heart_rate IS NULL OR max_heart_rate > resting_heart_rate)
    )
);

-- Create index for user profiles
CREATE INDEX IF NOT EXISTS idx_user_health_profiles_user_id ON user_health_profiles(user_id);