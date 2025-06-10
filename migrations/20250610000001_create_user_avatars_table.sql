CREATE TABLE IF NOT EXISTS user_avatars (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    stamina INTEGER NOT NULL DEFAULT 50,
    strength INTEGER NOT NULL DEFAULT 50,
    experience_points BIGINT NOT NULL DEFAULT 0,
    avatar_level INTEGER NOT NULL DEFAULT 1,
    avatar_style VARCHAR(50) DEFAULT 'warrior',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_user_avatar UNIQUE(user_id),
    CONSTRAINT reasonable_stats CHECK (
        stamina >= 0 AND stamina <= 200 AND
        strength >= 0 AND strength <= 200 AND
        experience_points >= 0
    )
);

CREATE INDEX IF NOT EXISTS idx_user_avatars_user_id ON user_avatars(user_id);
CREATE INDEX IF NOT EXISTS idx_user_avatars_stamina ON user_avatars(stamina DESC);
CREATE INDEX IF NOT EXISTS idx_user_avatars_strength ON user_avatars(strength DESC);
CREATE INDEX IF NOT EXISTS idx_user_avatars_experience ON user_avatars(experience_points DESC);