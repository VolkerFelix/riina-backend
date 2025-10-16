-- Backfill existing workouts as posts
-- This creates a post entry for each existing workout

BEGIN;

-- Insert posts for all existing public workouts
INSERT INTO posts (id, user_id, post_type, workout_id, visibility, is_editable, created_at, updated_at)
SELECT
    gen_random_uuid() as id,
    wd.user_id,
    'workout'::post_type,
    wd.id as workout_id,
    COALESCE(wd.visibility::post_visibility, 'public'::post_visibility),
    true as is_editable,
    COALESCE(wd.workout_start, wd.created_at) as created_at,
    wd.updated_at
FROM workout_data wd
WHERE NOT EXISTS (
    SELECT 1 FROM posts p WHERE p.workout_id = wd.id
);

COMMIT;
