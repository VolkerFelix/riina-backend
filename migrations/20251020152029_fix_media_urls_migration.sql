-- Fix media_urls migration to properly migrate existing data
-- The previous migration had invalid SQL syntax in the WHERE clauses

-- First, ensure the column exists (in case the previous migration partially succeeded)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'posts' AND column_name = 'media_urls') THEN
        ALTER TABLE posts ADD COLUMN media_urls JSONB;
    END IF;
END $$;

-- Migrate existing data from image_urls and video_urls to media_urls
-- This query correctly references the posts table's columns
UPDATE posts
SET media_urls = (
    SELECT jsonb_agg(media_item ORDER BY idx)
    FROM (
        -- Convert image_urls to media items
        SELECT
            row_number() OVER () as idx,
            jsonb_build_object('type', 'image', 'url', url) as media_item
        FROM unnest(COALESCE(posts.image_urls, ARRAY[]::text[])) WITH ORDINALITY AS t(url, ord)

        UNION ALL

        -- Convert video_urls to media items (offset by image count)
        SELECT
            (SELECT count(*) FROM unnest(COALESCE(posts.image_urls, ARRAY[]::text[]))) + row_number() OVER () as idx,
            jsonb_build_object('type', 'video', 'url', url) as media_item
        FROM unnest(COALESCE(posts.video_urls, ARRAY[]::text[])) WITH ORDINALITY AS t(url, ord)
    ) combined
)
WHERE (image_urls IS NOT NULL AND array_length(image_urls, 1) > 0)
   OR (video_urls IS NOT NULL AND array_length(video_urls, 1) > 0);

-- Create index if it doesn't exist
CREATE INDEX IF NOT EXISTS idx_posts_media_urls ON posts USING GIN (media_urls);

-- Add comment explaining the column
COMMENT ON COLUMN posts.media_urls IS 'Ordered array of media items with type and URL, replaces image_urls and video_urls to preserve mixed media order';
