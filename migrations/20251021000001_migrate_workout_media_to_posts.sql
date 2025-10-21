-- Migrate workout media (image_url, video_url) from workout_data to posts.media_urls
-- This ensures old workout posts show their images/videos in the new media_urls format

UPDATE posts p
SET media_urls = (
    SELECT jsonb_agg(media_item ORDER BY idx)
    FROM (
        -- Add workout image if exists
        SELECT 1 as idx, jsonb_build_object('type', 'image', 'url', wd.image_url) as media_item
        FROM workout_data wd
        WHERE wd.id = p.workout_id AND wd.image_url IS NOT NULL

        UNION ALL

        -- Add workout video if exists
        SELECT 2 as idx, jsonb_build_object('type', 'video', 'url', wd.video_url) as media_item
        FROM workout_data wd
        WHERE wd.id = p.workout_id AND wd.video_url IS NOT NULL
    ) combined
)
WHERE p.post_type = 'workout'
  AND p.workout_id IS NOT NULL
  AND (p.media_urls IS NULL OR p.media_urls = 'null'::jsonb OR jsonb_array_length(p.media_urls) = 0);
