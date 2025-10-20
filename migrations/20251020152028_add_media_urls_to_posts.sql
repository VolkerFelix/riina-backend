-- Add media_urls column to posts table to preserve mixed image/video order
-- media_urls will be a JSON array of objects like: [{"type": "image", "url": "..."}, {"type": "video", "url": "..."}]
-- This replaces the separate image_urls and video_urls arrays and preserves exact order

ALTER TABLE posts
ADD COLUMN media_urls JSONB;

-- Migrate existing data from image_urls and video_urls to media_urls
-- Concatenate images first, then videos (current behavior)
UPDATE posts
SET media_urls = (
    SELECT jsonb_agg(media_item ORDER BY idx)
    FROM (
        SELECT
            idx,
            jsonb_build_object('type', 'image', 'url', url) as media_item
        FROM (
            SELECT
                unnest(image_urls) as url,
                generate_series(1, array_length(image_urls, 1)) as idx
            WHERE image_urls IS NOT NULL
        ) images

        UNION ALL

        SELECT
            COALESCE(array_length(image_urls, 1), 0) + idx as idx,
            jsonb_build_object('type', 'video', 'url', url) as media_item
        FROM (
            SELECT
                unnest(video_urls) as url,
                generate_series(1, array_length(video_urls, 1)) as idx
            WHERE video_urls IS NOT NULL
        ) videos
    ) combined
)
WHERE image_urls IS NOT NULL OR video_urls IS NOT NULL;

-- Create index for querying media_urls
CREATE INDEX idx_posts_media_urls ON posts USING GIN (media_urls);

-- Comment explaining the column
COMMENT ON COLUMN posts.media_urls IS 'Ordered array of media items with type and URL, replaces image_urls and video_urls to preserve mixed media order';
