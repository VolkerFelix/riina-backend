-- Migration to fix media URL format in posts and workout_data tables
-- Replaces incorrectly migrated URLs that are missing the media/ prefix
-- This fixes URLs from {user_id}/{filename} to media/{user_id}/{filename}
-- Also handles any remaining /health/workout-media/ URLs

-- Fix image_urls in posts table - add media/ prefix to URLs without it
UPDATE posts
SET image_urls = ARRAY(
  SELECT CASE
    WHEN url LIKE '/health/workout-media/%' THEN regexp_replace(url, '^/health/workout-media/', 'media/')
    WHEN url NOT LIKE 'media/%' AND url ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/' THEN 'media/' || url
    ELSE url
  END
  FROM unnest(image_urls) AS url
)
WHERE image_urls IS NOT NULL
  AND (EXISTS (SELECT 1 FROM unnest(image_urls) AS url WHERE url LIKE '/health/workout-media/%')
       OR EXISTS (SELECT 1 FROM unnest(image_urls) AS url WHERE url NOT LIKE 'media/%' AND url ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/'));

-- Fix video_urls in posts table
UPDATE posts
SET video_urls = ARRAY(
  SELECT CASE
    WHEN url LIKE '/health/workout-media/%' THEN regexp_replace(url, '^/health/workout-media/', 'media/')
    WHEN url NOT LIKE 'media/%' AND url ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/' THEN 'media/' || url
    ELSE url
  END
  FROM unnest(video_urls) AS url
)
WHERE video_urls IS NOT NULL
  AND (EXISTS (SELECT 1 FROM unnest(video_urls) AS url WHERE url LIKE '/health/workout-media/%')
       OR EXISTS (SELECT 1 FROM unnest(video_urls) AS url WHERE url NOT LIKE 'media/%' AND url ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/'));

-- Fix image_url in workout_data table (singular, text field)
UPDATE workout_data
SET image_url = CASE
  WHEN image_url LIKE '/health/workout-media/%' THEN regexp_replace(image_url, '^/health/workout-media/', 'media/')
  WHEN image_url NOT LIKE 'media/%' AND image_url ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/' THEN 'media/' || image_url
  ELSE image_url
END
WHERE image_url IS NOT NULL
  AND (image_url LIKE '/health/workout-media/%'
       OR (image_url NOT LIKE 'media/%' AND image_url ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/'));

-- Fix video_url in workout_data table (singular, text field)
UPDATE workout_data
SET video_url = CASE
  WHEN video_url LIKE '/health/workout-media/%' THEN regexp_replace(video_url, '^/health/workout-media/', 'media/')
  WHEN video_url NOT LIKE 'media/%' AND video_url ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/' THEN 'media/' || video_url
  ELSE video_url
END
WHERE video_url IS NOT NULL
  AND (video_url LIKE '/health/workout-media/%'
       OR (video_url NOT LIKE 'media/%' AND video_url ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/'));
