-- Create posts table for unified content feed
-- Posts can reference workouts, contain ad data, or be standalone content

BEGIN;

-- Create post_type enum
CREATE TYPE post_type AS ENUM ('workout', 'ad', 'universal');

-- Create visibility enum (reuse workout visibility logic)
CREATE TYPE post_visibility AS ENUM ('public', 'friends', 'private');

-- Create posts table
CREATE TABLE posts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    post_type post_type NOT NULL DEFAULT 'universal',

    -- Content fields
    content TEXT, -- Caption, universal content, or ad copy

    -- References
    workout_id UUID REFERENCES workout_data(id) ON DELETE CASCADE, -- For workout posts

    -- Media (arrays for multiple attachments)
    image_urls TEXT[], -- Array of image URLs
    video_urls TEXT[], -- Array of video URLs

    -- Ad metadata (JSON for flexibility)
    ad_metadata JSONB, -- Ad network data, placement info, etc.

    -- Visibility and permissions
    visibility post_visibility NOT NULL DEFAULT 'public',
    is_editable BOOLEAN NOT NULL DEFAULT true,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    edited_at TIMESTAMPTZ, -- Track when user edited the post

    -- Constraints
    CONSTRAINT workout_posts_must_have_workout_id
        CHECK (post_type != 'workout' OR workout_id IS NOT NULL),
    CONSTRAINT ad_posts_must_have_metadata
        CHECK (post_type != 'ad' OR ad_metadata IS NOT NULL)
);

-- Indexes for performance
CREATE INDEX idx_posts_user_id ON posts(user_id);
CREATE INDEX idx_posts_workout_id ON posts(workout_id) WHERE workout_id IS NOT NULL;
CREATE INDEX idx_posts_created_at ON posts(created_at DESC);
CREATE INDEX idx_posts_type ON posts(post_type);
CREATE INDEX idx_posts_visibility ON posts(visibility);

-- Composite index for feed queries
CREATE INDEX idx_posts_feed_query ON posts(visibility, created_at DESC)
    WHERE visibility = 'public';

-- Comments
COMMENT ON TABLE posts IS 'Unified posts table for feed content including workouts, ads, and universal posts';
COMMENT ON COLUMN posts.post_type IS 'Type of post: workout, ad, or universal';
COMMENT ON COLUMN posts.content IS 'User-written caption or universal content';
COMMENT ON COLUMN posts.workout_id IS 'Reference to workout_data for workout posts';
COMMENT ON COLUMN posts.image_urls IS 'Array of image URLs for multi-image posts';
COMMENT ON COLUMN posts.video_urls IS 'Array of video URLs for multi-video posts';
COMMENT ON COLUMN posts.ad_metadata IS 'JSON metadata for ad posts (network, placement, targeting)';
COMMENT ON COLUMN posts.is_editable IS 'Whether the user can edit this post';
COMMENT ON COLUMN posts.edited_at IS 'Timestamp of last user edit (NULL if never edited)';

COMMIT;
