-- Add label column to workout_data for ML pipeline classification
-- Label indicates the true workout type (strength, cardio, etc.)

ALTER TABLE workout_data
ADD COLUMN label VARCHAR(50);

-- Create index for label for efficient filtering in ML pipeline
CREATE INDEX idx_workout_data_label ON workout_data(label) WHERE label IS NOT NULL;
