-- Migration to fix media URL format in posts and workout_data tables
-- Removes legacy /health/workout-media/ prefix from image_urls and video_urls
-- New format: {user_id}/{filename} instead of /health/workout-media/{user_id}/{filename}

-- Fix image_urls in posts table
UPDATE posts
SET image_urls = ARRAY(
  SELECT regexp_replace(url, '^/health/workout-media/', '')
  FROM unnest(image_urls) AS url
)
WHERE image_urls IS NOT NULL
  AND EXISTS (SELECT 1 FROM unnest(image_urls) AS url WHERE url LIKE '/health/workout-media/%');

-- Fix video_urls in posts table
UPDATE posts
SET video_urls = ARRAY(
  SELECT regexp_replace(url, '^/health/workout-media/', '')
  FROM unnest(video_urls) AS url
)
WHERE video_urls IS NOT NULL
  AND EXISTS (SELECT 1 FROM unnest(video_urls) AS url WHERE url LIKE '/health/workout-media/%');

-- Fix image_url in workout_data table (singular, text field)
UPDATE workout_data
SET image_url = regexp_replace(image_url, '^/health/workout-media/', '')
WHERE image_url IS NOT NULL
  AND image_url LIKE '/health/workout-media/%';

-- Fix video_url in workout_data table (singular, text field)
UPDATE workout_data
SET video_url = regexp_replace(video_url, '^/health/workout-media/', '')
WHERE video_url IS NOT NULL
  AND video_url LIKE '/health/workout-media/%';
