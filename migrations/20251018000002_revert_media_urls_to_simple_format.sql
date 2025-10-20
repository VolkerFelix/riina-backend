-- Migration to revert media URLs back to simple {user_id}/{filename} format
-- This removes the media/ prefix that was incorrectly added
-- The backend handler will add the media/ prefix when constructing MinIO paths

-- Fix image_urls in posts table - remove media/ prefix
UPDATE posts
SET image_urls = ARRAY(
  SELECT regexp_replace(url, '^media/', '')
  FROM unnest(image_urls) AS url
)
WHERE image_urls IS NOT NULL
  AND EXISTS (SELECT 1 FROM unnest(image_urls) AS url WHERE url LIKE 'media/%');

-- Fix video_urls in posts table
UPDATE posts
SET video_urls = ARRAY(
  SELECT regexp_replace(url, '^media/', '')
  FROM unnest(video_urls) AS url
)
WHERE video_urls IS NOT NULL
  AND EXISTS (SELECT 1 FROM unnest(video_urls) AS url WHERE url LIKE 'media/%');

-- Fix image_url in workout_data table (singular, text field)
UPDATE workout_data
SET image_url = regexp_replace(image_url, '^media/', '')
WHERE image_url IS NOT NULL
  AND image_url LIKE 'media/%';

-- Fix video_url in workout_data table (singular, text field)
UPDATE workout_data
SET video_url = regexp_replace(video_url, '^media/', '')
WHERE video_url IS NOT NULL
  AND video_url LIKE 'media/%';
