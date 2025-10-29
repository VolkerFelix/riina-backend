#!/bin/bash

# Script to reset all users' max heart rates using the calculation formulas
# This will recalculate max heart rate based on age and gender, and update VT thresholds
# Usage: ./reset_max_heart_rates_fly.sh

set -e

echo "🚀 Resetting max heart rates for all users on Fly.io database..."
echo "📋 This will recalculate max heart rate based on age and gender"
echo "🔧 VT thresholds will also be recalculated"
echo ""

# Create a temporary SQL file
TEMP_SQL=$(mktemp)
cat > "$TEMP_SQL" << 'EOF'
-- Reset max heart rates and recalculate VT thresholds for all users
-- Based on the calculation formulas from health_calculations.rs

WITH updated_profiles AS (
    UPDATE user_health_profiles 
    SET 
        max_heart_rate = CASE 
            -- Male formulas
            WHEN gender = 'male' OR gender = 'm' THEN
                CASE 
                    WHEN age >= 40 THEN (216.0 - (0.93 * age))::INTEGER
                    ELSE (208.0 - (0.7 * age))::INTEGER
                END
            -- Female formulas  
            WHEN gender = 'female' OR gender = 'f' THEN
                CASE 
                    WHEN age >= 40 THEN (200.0 - (0.67 * age))::INTEGER
                    ELSE (206.0 - (0.88 * age))::INTEGER
                END
            -- Other/Default formula
            ELSE (208.0 - (0.7 * age))::INTEGER
        END,
        -- Recalculate VT thresholds based on new max heart rate
        vt0_threshold = CASE 
            WHEN gender = 'male' OR gender = 'm' THEN
                CASE 
                    WHEN age >= 40 THEN 
                        COALESCE(resting_heart_rate, 60) + ((216.0 - (0.93 * age) - COALESCE(resting_heart_rate, 60)) * 0.35)::INTEGER
                    ELSE 
                        COALESCE(resting_heart_rate, 60) + ((208.0 - (0.7 * age) - COALESCE(resting_heart_rate, 60)) * 0.35)::INTEGER
                END
            WHEN gender = 'female' OR gender = 'f' THEN
                CASE 
                    WHEN age >= 40 THEN 
                        COALESCE(resting_heart_rate, 60) + ((200.0 - (0.67 * age) - COALESCE(resting_heart_rate, 60)) * 0.35)::INTEGER
                    ELSE 
                        COALESCE(resting_heart_rate, 60) + ((206.0 - (0.88 * age) - COALESCE(resting_heart_rate, 60)) * 0.35)::INTEGER
                END
            ELSE 
                COALESCE(resting_heart_rate, 60) + ((208.0 - (0.7 * age) - COALESCE(resting_heart_rate, 60)) * 0.35)::INTEGER
        END,
        vt1_threshold = CASE 
            WHEN gender = 'male' OR gender = 'm' THEN
                CASE 
                    WHEN age >= 40 THEN 
                        COALESCE(resting_heart_rate, 60) + ((216.0 - (0.93 * age) - COALESCE(resting_heart_rate, 60)) * 0.65)::INTEGER
                    ELSE 
                        COALESCE(resting_heart_rate, 60) + ((208.0 - (0.7 * age) - COALESCE(resting_heart_rate, 60)) * 0.65)::INTEGER
                END
            WHEN gender = 'female' OR gender = 'f' THEN
                CASE 
                    WHEN age >= 40 THEN 
                        COALESCE(resting_heart_rate, 60) + ((200.0 - (0.67 * age) - COALESCE(resting_heart_rate, 60)) * 0.65)::INTEGER
                    ELSE 
                        COALESCE(resting_heart_rate, 60) + ((206.0 - (0.88 * age) - COALESCE(resting_heart_rate, 60)) * 0.65)::INTEGER
                END
            ELSE 
                COALESCE(resting_heart_rate, 60) + ((208.0 - (0.7 * age) - COALESCE(resting_heart_rate, 60)) * 0.65)::INTEGER
        END,
        vt2_threshold = CASE 
            WHEN gender = 'male' OR gender = 'm' THEN
                CASE 
                    WHEN age >= 40 THEN 
                        COALESCE(resting_heart_rate, 60) + ((216.0 - (0.93 * age) - COALESCE(resting_heart_rate, 60)) * 0.8)::INTEGER
                    ELSE 
                        COALESCE(resting_heart_rate, 60) + ((208.0 - (0.7 * age) - COALESCE(resting_heart_rate, 60)) * 0.8)::INTEGER
                END
            WHEN gender = 'female' OR gender = 'f' THEN
                CASE 
                    WHEN age >= 40 THEN 
                        COALESCE(resting_heart_rate, 60) + ((200.0 - (0.67 * age) - COALESCE(resting_heart_rate, 60)) * 0.8)::INTEGER
                    ELSE 
                        COALESCE(resting_heart_rate, 60) + ((206.0 - (0.88 * age) - COALESCE(resting_heart_rate, 60)) * 0.8)::INTEGER
                END
            ELSE 
                COALESCE(resting_heart_rate, 60) + ((208.0 - (0.7 * age) - COALESCE(resting_heart_rate, 60)) * 0.8)::INTEGER
        END,
        last_updated = NOW()
    WHERE age IS NOT NULL 
        AND gender IS NOT NULL
        AND (age >= 10 AND age <= 120)  -- Reasonable age range
    RETURNING 
        user_id,
        age,
        gender,
        resting_heart_rate,
        max_heart_rate,
        vt0_threshold,
        vt1_threshold,
        vt2_threshold
)
SELECT 
    COUNT(*) as users_updated,
    'Max heart rates and VT thresholds recalculated' as status
FROM updated_profiles;

-- Show summary of updated profiles
SELECT 
    'Summary of updated profiles:' as info,
    COUNT(*) as total_users,
    AVG(age)::INTEGER as avg_age,
    COUNT(CASE WHEN gender IN ('male', 'm') THEN 1 END) as male_users,
    COUNT(CASE WHEN gender IN ('female', 'f') THEN 1 END) as female_users,
    COUNT(CASE WHEN gender NOT IN ('male', 'm', 'female', 'f') THEN 1 END) as other_users,
    AVG(max_heart_rate)::INTEGER as avg_max_hr,
    AVG(vt0_threshold)::INTEGER as avg_vt0,
    AVG(vt1_threshold)::INTEGER as avg_vt1,
    AVG(vt2_threshold)::INTEGER as avg_vt2
FROM user_health_profiles 
WHERE max_heart_rate IS NOT NULL;

-- Show some example calculations
SELECT 
    'Example calculations:' as info,
    user_id,
    age,
    gender,
    resting_heart_rate,
    max_heart_rate,
    vt0_threshold,
    vt1_threshold,
    vt2_threshold
FROM user_health_profiles 
WHERE max_heart_rate IS NOT NULL
ORDER BY age
LIMIT 10;
EOF

# Execute the SQL commands
echo "⚡ Updating max heart rates and VT thresholds..."
echo ""

fly postgres connect -a evolveme-db -d evolveme_db < "$TEMP_SQL"

# Clean up
rm -f "$TEMP_SQL"

echo ""
echo "✅ Max heart rate reset completed!"
echo "💡 All users' max heart rates have been recalculated based on age and gender"
echo "🔧 VT thresholds have been updated accordingly"
echo ""
echo "📊 Formulas used:"
echo "   Men <40:  HR Max = 208 - (0.7 × age)"
echo "   Men 40+:  HR Max = 216 - (0.93 × age)"
echo "   Women <40: HR Max = 206 - (0.88 × age)"
echo "   Women 40+: HR Max = 200 - (0.67 × age)"
echo "   Other:     HR Max = 208 - (0.7 × age)"
echo ""
echo "🎯 VT Thresholds:"
echo "   VT0 (Aerobic Base): 35% of Heart Rate Reserve"
echo "   VT1 (Aerobic Threshold): 65% of Heart Rate Reserve"
echo "   VT2 (Lactate Threshold): 80% of Heart Rate Reserve"
