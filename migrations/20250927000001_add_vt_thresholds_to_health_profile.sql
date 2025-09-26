-- Add VT threshold columns to user health profiles
-- VT0 = Aerobic Base (Zone 1/2 boundary)
-- VT1 = Aerobic Threshold (Zone 2/3 boundary)
-- VT2 = Lactate Threshold (Zone 3/4 boundary)

ALTER TABLE user_health_profiles
ADD COLUMN vt0_threshold INTEGER, -- VT0: Aerobic Base threshold
ADD COLUMN vt1_threshold INTEGER, -- VT1: Aerobic threshold
ADD COLUMN vt2_threshold INTEGER; -- VT2: Lactate threshold