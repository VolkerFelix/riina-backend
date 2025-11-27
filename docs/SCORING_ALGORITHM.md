# Workout Scoring Algorithm

This document explains how Riina.io calculates stamina points from workout heart rate data using a ventilatory threshold-based system.

## Overview

Riina.io uses a **Universal HR-Based Scoring** method that calculates workout performance based on ventilatory thresholds (VT0, VT1, VT2). This scientific approach aligns with how your body actually responds to exercise intensity, using physiological markers rather than arbitrary percentages.

---

## Key Concepts

### Resting Heart Rate (HR Rest)
Your heart rate when completely at rest, typically measured first thing in the morning before getting out of bed.

- **Average:** 60-100 bpm for adults
- **Athletes:** Often 40-60 bpm
- **How to measure:** Take your pulse for 60 seconds upon waking for 3-5 consecutive days and use the average

### Maximum Heart Rate (HR Max)
The highest heart rate you can achieve during maximal exercise. Calculated using age and gender-specific formulas:

**Men:**
- Under 40: `HR Max = 208 - (0.7 × age)`
- 40+: `HR Max = 216 - (0.93 × age)`

**Women:**
- Under 40: `HR Max = 206 - (0.88 × age)`
- 40+: `HR Max = 200 - (0.67 × age)`

**Other/Default:**
- `HR Max = 208 - (0.7 × age)`

**Example:** A 35-year-old male:
```
HR Max = 208 - (0.7 × 35) = 208 - 24.5 = 183.5 ≈ 183 bpm
```

### Heart Rate Reserve (HRR)
The difference between your maximum and resting heart rates. This represents your "usable" heart rate range for exercise:

```
HRR = HR Max - HR Rest
```

**Example:** If HR Max = 183 and HR Rest = 60:
```
HRR = 183 - 60 = 123 bpm
```

This 123 bpm range is what you have available for exercise intensity variation.

---

## Ventilatory Thresholds

Ventilatory thresholds are physiological markers where your breathing pattern and metabolism change during progressively harder exercise. They represent real shifts in how your body produces energy.

### VT_OFF: No Training Effect (20% of HRR)

**Calculation:** `VT_OFF = HR Rest + (HRR × 0.20)`

**Physiological meaning:**
- Heart rate only slightly elevated from resting
- No significant cardiovascular training stimulus
- Normal daily activities, light movement
- Essentially resting metabolism

**Training benefit:** None - below the threshold for cardiovascular adaptation

**Feel:** Barely noticeable physical effort, normal daily activities

### VT0: Aerobic Threshold (35% of HRR)

**Calculation:** `VT0 = HR Rest + (HRR × 0.35)`

**Physiological meaning:**
- First noticeable increase in breathing rate
- Fat metabolism is maximized
- Lactate production is minimal
- You can maintain this intensity for hours

**Training benefit:** Base aerobic fitness, fat adaptation, recovery

**Feel:** Very comfortable, easy conversation, could sustain "all day"

### VT1: Lactate Threshold (65% of HRR)

**Calculation:** `VT1 = HR Rest + (HRR × 0.65)`

**Physiological meaning:**
- Breathing becomes noticeably labored
- Point where lactate begins accumulating faster than it can be cleared
- Transition from mostly aerobic to mixed aerobic/anaerobic metabolism
- Sustainable for 30-60 minutes

**Training benefit:** Improves lactate clearance, increases sustainable pace

**Feel:** Moderately hard, can speak in short sentences, "comfortably hard"

### VT2: Anaerobic Threshold (80% of HRR)

**Calculation:** `VT2 = HR Rest + (HRR × 0.80)`

**Physiological meaning:**
- Heavy, rapid breathing
- Rapid lactate accumulation, approaching VO2 max
- Mostly anaerobic metabolism
- Sustainable for only 2-8 minutes

**Training benefit:** Increases VO2 max, improves top-end power

**Feel:** Very hard, cannot speak, maximal sustainable effort

---

## Complete Example Calculation

Let's calculate the thresholds for our 35-year-old male athlete:

**Given:**
- Age: 35
- Gender: Male
- HR Rest: 60 bpm (measured)

**Step 1: Calculate HR Max**
```
HR Max = 208 - (0.7 × 35)
HR Max = 208 - 24.5
HR Max = 183 bpm
```

**Step 2: Calculate HRR**
```
HRR = HR Max - HR Rest
HRR = 183 - 60
HRR = 123 bpm
```

**Step 3: Calculate Thresholds**
```
VT_OFF = 60 + (123 × 0.20) = 60 + 24.60 = 85 bpm
VT0 = 60 + (123 × 0.35) = 60 + 43.05 = 103 bpm
VT1 = 60 + (123 × 0.65) = 60 + 79.95 = 140 bpm
VT2 = 60 + (123 × 0.80) = 60 + 98.40 = 158 bpm
```

**Summary:**
- **HR Rest:** 60 bpm
- **VT_OFF:** 85 bpm (no training effect threshold)
- **VT0:** 103 bpm (aerobic threshold)
- **VT1:** 140 bpm (lactate threshold)
- **VT2:** 158 bpm (anaerobic threshold)
- **HR Max:** 183 bpm

---

## Training Zones

The four ventilatory thresholds divide your heart rate range into 5 distinct training zones:

| Zone | Heart Rate Range | Intensity | Points/min | Purpose |
|------|------------------|-----------|------------|---------|
| **OFF** | 0 → VT_OFF (0-84 bpm) | 0.0 | 0 | No training effect, daily activities |
| **REST** | VT_OFF → VT0 (85-102 bpm) | 1.0 | 1 | Active recovery, warm-up |
| **EASY** | VT0 → VT1 (103-139 bpm) | 4.0 | 4 | Aerobic base building |
| **MODERATE** | VT1 → VT2 (140-157 bpm) | 6.0 | 6 | Lactate threshold training |
| **HARD** | VT2 → Max (158-183 bpm) | 8.0 | 8 | VO2 max, anaerobic power |

### Zone Descriptions

**OFF Zone (0 - VT_OFF)**
- **Heart Rate:** Below training threshold
- **Metabolism:** Resting/minimal activity
- **Duration:** All day
- **Examples:** Sitting, standing, slow walking, daily activities
- **Points:** 0 per minute - no training effect

**REST Zone (VT_OFF - VT0)**
- **Heart Rate:** Below aerobic threshold but above resting
- **Metabolism:** Primarily fat burning, minimal carbohydrate use
- **Duration:** Can maintain indefinitely
- **Examples:** Brisk walking, very light cycling, active recovery
- **Points:** 1 per minute

**EASY Zone (VT0 - VT1)**
- **Heart Rate:** Between aerobic and lactate thresholds
- **Metabolism:** Optimal aerobic, significant fat contribution
- **Duration:** Can maintain for many hours
- **Examples:** Easy jogging, conversational pace cycling
- **Points:** 4 per minute
- **Training note:** This is where most endurance training should occur

**MODERATE Zone (VT1 - VT2)**
- **Heart Rate:** Between lactate and anaerobic thresholds
- **Metabolism:** Mixed aerobic/anaerobic, lactate accumulation manageable
- **Duration:** 30-60 minutes
- **Examples:** Tempo runs, steady-state intervals
- **Points:** 6 per minute
- **Training note:** Improves race pace and lactate clearance

**HARD Zone (VT2 - Max)**
- **Heart Rate:** Above anaerobic threshold
- **Metabolism:** Heavily anaerobic, rapid lactate buildup
- **Duration:** 2-8 minutes maximum
- **Examples:** VO2 max intervals, sprint finishes
- **Points:** 8 per minute
- **Training note:** Use sparingly, requires significant recovery

---

## Points Calculation

The scoring system awards **stamina points** based on time spent in each training zone, weighted by intensity.

### Formula

```
Total Stamina Points = Σ (time_in_zone_minutes × zone_intensity_multiplier)
```

For each heart rate sample in your workout:
1. Determine which zone it falls into based on the thresholds
2. Calculate the duration until the next sample
3. Add points: `duration_minutes × zone_intensity`
4. Sum all points across the entire workout

### Intensity Multipliers

- **REST:** 1.0 points per minute
- **EASY:** 4.0 points per minute
- **MODERATE:** 6.0 points per minute
- **HARD:** 8.0 points per minute

**Design rationale:** Higher intensities earn more points per minute, but the total points depend on how long you can sustain each intensity. A 60-minute easy run may score more total points than a 10-minute hard interval session.

---

## Worked Example: 45-Minute Run

Using our 35-year-old male athlete (VT0=103, VT1=140, VT2=158):

**Workout breakdown:**
- 5 minutes warm-up in REST zone (avg HR 95 bpm)
- 25 minutes in EASY zone (avg HR 130 bpm)
- 10 minutes in MODERATE zone (avg HR 150 bpm)
- 5 minutes in HARD zone (avg HR 165 bpm)

**Points calculation:**

| Zone | Time (min) | Intensity | Calculation | Points |
|------|------------|-----------|-------------|---------|
| REST | 5 | 1.0 | 5 × 1.0 | 5 |
| EASY | 25 | 4.0 | 25 × 4.0 | 100 |
| MODERATE | 10 | 6.0 | 10 × 6.0 | 60 |
| HARD | 5 | 8.0 | 5 × 8.0 | 40 |
| **TOTAL** | **45** | - | - | **205** |

**Result:** This 45-minute workout earns **205 stamina points**.

### Alternative Workout Comparison

**Option A:** 45 minutes steady EASY zone (130 bpm)
```
45 × 4.0 = 180 points
```

**Option B:** 30 minutes EASY + 15 minutes MODERATE
```
(30 × 4.0) + (15 × 6.0) = 120 + 90 = 210 points
```

**Option C:** 10 minutes HARD intervals with rest
```
(10 × 8.0) + (35 × 1.0) = 80 + 35 = 115 points
```

**Insight:** Longer, moderate-intensity workouts typically score more total points than short, intense efforts. However, different intensities provide different physiological adaptations.

---

## Heart Rate Data Format

The system processes heart rate data as a time series of samples:

```json
[
  {"timestamp": "2025-01-15T10:00:00Z", "heart_rate": 95},
  {"timestamp": "2025-01-15T10:01:00Z", "heart_rate": 110},
  {"timestamp": "2025-01-15T10:02:00Z", "heart_rate": 128},
  {"timestamp": "2025-01-15T10:03:00Z", "heart_rate": 135},
  ...
]
```

**Processing:**
1. For each consecutive pair of samples, calculate the time interval
2. Assign the interval to the zone of the current sample
3. Calculate points: `(interval_seconds / 60) × zone_intensity`
4. Sum all intervals across the entire workout

---

## System Output

After processing a workout, the system returns:

```json
{
  "stamina_gained": 205.0,
  "strength_gained": 0.0,
  "zone_breakdown": [
    {
      "zone": "Rest",
      "minutes": 5.0,
      "stamina_gained": 5.0,
      "strength_gained": 0.0,
      "hr_min": null,
      "hr_max": 103
    },
    {
      "zone": "Easy",
      "minutes": 25.0,
      "stamina_gained": 100.0,
      "strength_gained": 0.0,
      "hr_min": 103,
      "hr_max": 140
    },
    {
      "zone": "Moderate",
      "minutes": 10.0,
      "stamina_gained": 60.0,
      "strength_gained": 0.0,
      "hr_min": 140,
      "hr_max": 158
    },
    {
      "zone": "Hard",
      "minutes": 5.0,
      "stamina_gained": 40.0,
      "strength_gained": 0.0,
      "hr_min": 158,
      "hr_max": 183
    }
  ]
}
```

**Note:** This method only calculates stamina points. Strength gains are not calculated in the VT-based system.

---

## Constants Reference

All constants used in the algorithm:

```rust
// Ventilatory threshold percentages (% of Heart Rate Reserve)
pub const P_VT_OFF: f32 = 0.20;  // No training effect threshold
pub const P_VT0: f32 = 0.35;     // Aerobic threshold
pub const P_VT1: f32 = 0.65;     // Lactate threshold
pub const P_VT2: f32 = 0.80;     // Anaerobic threshold

// Training zone intensity multipliers (points per minute)
OFF_ZONE_INTENSITY       = 0.0
REST_ZONE_INTENSITY      = 1.0
EASY_ZONE_INTENSITY      = 4.0
MODERATE_ZONE_INTENSITY  = 6.0
HARD_ZONE_INTENSITY      = 8.0

// Max HR formulas by age and gender
Men <40:    HR_Max = 208 - (0.7 × age)
Men 40+:    HR_Max = 216 - (0.93 × age)
Women <40:  HR_Max = 206 - (0.88 × age)
Women 40+:  HR_Max = 200 - (0.67 × age)
Default:    HR_Max = 208 - (0.7 × age)
```

---

## Code Implementation

### Key Files

1. **Scoring Logic:** [`src/workout/universal_hr_based_scoring.rs`](../src/workout/universal_hr_based_scoring.rs)
   - Main calculation function
   - Zone assignment
   - Points accumulation

2. **Zone Definitions:** [`src/models/health.rs`](../src/models/health.rs)
   - `TrainingZones` struct
   - `TrainingZone` with intensity calculation
   - VT threshold calculations

3. **Max HR Calculation:** [`src/utils/health_calculations.rs`](../src/utils/health_calculations.rs)
   - Age and gender-specific formulas
   - `calc_max_heart_rate()` function

### Algorithm Flow

```
1. Receive workout upload with HR data
   ↓
2. Load user health profile (age, gender, HR rest, HR max)
   ↓
3. Calculate HR Reserve (HRR = HR Max - HR Rest)
   ↓
4. Calculate VT thresholds (VT0, VT1, VT2)
   ↓
5. Create training zones with intensity multipliers
   ↓
6. Process HR samples in sequence:
   - For each sample pair:
     * Determine zone of current sample
     * Calculate time interval to next sample
     * Add points: interval × zone_intensity
   ↓
7. Aggregate points by zone
   ↓
8. Return total stamina + zone breakdown
```

---

## Advantages of VT-Based Scoring

### Physiologically Accurate
- Based on actual metabolic thresholds, not arbitrary percentages
- Aligns with how your body produces and uses energy
- Reflects real changes in breathing and lactate dynamics

### Scientifically Validated
- VT0, VT1, VT2 are used in exercise physiology research
- Corresponds to lab-tested thresholds (lactate testing, gas exchange analysis)
- Training at these intensities produces predictable adaptations

### Individualized
- Accounts for age, gender, and fitness level (via resting HR)
- Higher fitness athletes have lower resting HR → wider zones
- Zones shift as fitness improves

### Training Guidance
- Clear boundaries for different training stimuli
- EASY zone is optimal for most endurance training
- MODERATE zone builds race pace and lactate clearance
- HARD zone improves VO2 max but requires recovery

---

## FAQ

**Q: Why only stamina points and not strength?**

A: The VT-based system is designed for cardiovascular/endurance training. It measures aerobic capacity and metabolic efficiency, which translate to stamina. Strength gains require resistance training or specific power-based metrics not captured by steady-state heart rate.

---

**Q: What if I don't know my resting heart rate?**

A: The system defaults to 60 bpm if not provided. However, this affects zone accuracy significantly. For best results:
1. Measure HR immediately upon waking (before getting out of bed)
2. Do this for 3-5 consecutive mornings
3. Use the average value
4. Update your profile with this value

---

**Q: Can I use a fitness tracker or smartwatch?**

A: Yes! Most devices provide:
- Continuous heart rate monitoring
- Resting HR estimates (often more accurate than manual)
- Max HR detection during workouts
- Export data in compatible formats

---

**Q: Why do easy workouts sometimes score more than hard intervals?**

A: Total points = intensity × duration. A 60-minute easy run (60 × 4 = 240 points) can outscore a 10-minute hard interval session (10 × 8 = 80 points). Both provide valuable but different training stimuli:
- Easy = builds aerobic base, improves fat metabolism
- Hard = improves VO2 max, increases top-end power

Training should include both!

---

**Q: How do the percentages (35%, 65%, 80%) relate to traditional training zones?**

A: Traditional 5-zone systems often use 60%, 70%, 80%, 90% of HRR. The VT percentages are based on physiological markers, not round numbers:
- **VT0 (35%)** ≈ Traditional Zone 1/2 boundary
- **VT1 (65%)** ≈ Traditional Zone 2/3 boundary (tempo)
- **VT2 (80%)** ≈ Traditional Zone 3/4 boundary (threshold)

VT zones are wider and fewer because they represent distinct metabolic states.

---

**Q: What if my workout spans multiple zones?**

A: That's normal and expected! The algorithm automatically:
1. Analyzes every heart rate sample
2. Assigns it to the appropriate zone
3. Calculates duration in each zone
4. Sums points across all zones

The zone breakdown shows exactly how much time you spent in each intensity.

---

**Q: Does the algorithm account for heart rate drift?**

A: Yes, implicitly. Heart rate drift (HR increasing at constant effort due to fatigue, heat, dehydration) is reflected in the data. As your HR drifts upward:
- You move into higher zones
- You earn more points per minute
- This accurately reflects increased physiological stress

---

**Q: How often should I update my max heart rate?**

A: Max HR changes slowly with age (~1 bpm per year). Update when:
- You have a new max HR observation from a hard workout
- Your age changes significantly (5+ years)
- You notice zones feel significantly easier/harder than expected

---

**Q: Can I manually adjust the VT percentages?**

A: Currently the percentages (35%, 65%, 80%) are fixed based on exercise science research. These represent typical physiological thresholds for most people. Lab testing can determine your personal thresholds more precisely.

---

## Further Reading

**Scientific Background:**
- Ventilatory threshold concepts in exercise physiology
- Heart rate reserve method (Karvonen formula)
- Lactate threshold training
- VO2 max and anaerobic threshold

**Training Resources:**
- Polarized training models (80/20 rule)
- Heart rate-based training plans
- Zone 2 training for endurance athletes
- Interval training at threshold intensities

---

## Summary

The Universal HR-Based Scoring algorithm:

✅ Uses **four ventilatory thresholds** (VT_OFF, VT0, VT1, VT2) to define **five training zones**

✅ Calculates zones based on **Heart Rate Reserve** (HR Max - HR Rest)

✅ Awards **stamina points** based on **time × intensity** in each zone

✅ Provides **individualized** zones based on age, gender, and fitness level

✅ Delivers **physiologically accurate** training intensity feedback

✅ Generates **detailed zone breakdowns** for every workout

The system converts raw heart rate data into actionable training metrics, helping athletes optimize their endurance training and track cardiovascular fitness improvements over time.
