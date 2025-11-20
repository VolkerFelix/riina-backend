-- Change ml_confidence from DECIMAL to REAL for better compatibility with Rust f32
-- DECIMAL requires the bigdecimal feature in SQLx, but REAL maps directly to f32

ALTER TABLE workout_data
ALTER COLUMN ml_confidence TYPE REAL;
