-- Create posts for existing workouts that don't have associated posts
-- This ensures every workout has a corresponding post for editing functionality

INSERT INTO posts (id, user_id, post_type, workout_id, visibility, is_editable, created_at, updated_at)
SELECT 
    gen_random_uuid() as id,
    wd.user_id,
    'workout'::post_type,
    wd.id as workout_id,
    'public'::post_visibility,
    true as is_editable,
    wd.created_at,
    wd.created_at as updated_at
FROM workout_data wd
LEFT JOIN posts p ON p.workout_id = wd.id
WHERE p.id IS NULL
AND wd.user_id IS NOT NULL;
