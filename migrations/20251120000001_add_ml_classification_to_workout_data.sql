-- Add ML classification columns to workout_data
-- ml_prediction: The predicted workout type from the ML model
-- ml_confidence: Confidence score of the prediction (0-1)
-- ml_classified_at: Timestamp when ML classification was performed

ALTER TABLE workout_data
ADD COLUMN ml_prediction VARCHAR(50),
ADD COLUMN ml_confidence DECIMAL(5,4),
ADD COLUMN ml_classified_at TIMESTAMP WITH TIME ZONE;

-- Create index for ML prediction for efficient filtering
CREATE INDEX idx_workout_data_ml_prediction ON workout_data(ml_prediction) WHERE ml_prediction IS NOT NULL;

-- Create index for high-confidence predictions
CREATE INDEX idx_workout_data_ml_confidence ON workout_data(ml_confidence) WHERE ml_confidence > 0.8;
