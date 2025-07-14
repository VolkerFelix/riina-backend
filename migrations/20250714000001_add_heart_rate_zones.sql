-- Add heart rate zone thresholds to user health profiles
-- These will be calculated and set by the application code

ALTER TABLE user_health_profiles 
ADD COLUMN max_heart_rate INTEGER,
ADD COLUMN hr_zone_1_max INTEGER, -- Zone 1: Recovery 
ADD COLUMN hr_zone_2_max INTEGER, -- Zone 2: Aerobic Base
ADD COLUMN hr_zone_3_max INTEGER, -- Zone 3: Aerobic
ADD COLUMN hr_zone_4_max INTEGER, -- Zone 4: Threshold
ADD COLUMN hr_zone_5_max INTEGER; -- Zone 5: VO2 Max

-- Add constraint for reasonable max heart rate values
ALTER TABLE user_health_profiles 
ADD CONSTRAINT reasonable_max_heart_rate CHECK (
    max_heart_rate IS NULL OR (max_heart_rate >= 100 AND max_heart_rate <= 250)
);