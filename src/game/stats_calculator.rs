use std::pin::Pin;
use std::future::Future;
use std::io::Error;

use crate::models::{
    workout_data::{WorkoutStats, HeartRateData, WorkoutType},
    health::UserHealthProfile,
};
use crate::workout::{
    universal_hr_based_scoring::UniversalHRBasedScoring,
};  
// Method trait for different scoring methods
pub trait ScoringMethod {
    fn calculate_stats(
        &self,
        user_health_profile: UserHealthProfile,
        hr_data: Vec<HeartRateData>,
        workout_type: WorkoutType
    ) -> Pin<Box<dyn Future<Output = Result<WorkoutStats, Error>> + Send + 'static>>;
}

pub struct WorkoutStatsCalculator {
    scoring_method: Box<dyn ScoringMethod + Send + Sync>,
}

impl WorkoutStatsCalculator {
    pub fn new(scoring_method: Box<dyn ScoringMethod + Send + Sync>) -> Self {
        Self { scoring_method }
    }
    
    pub fn with_universal_hr_based() -> Self {
        Self::new(Box::new(UniversalHRBasedScoring))
    }
    
    pub async fn calculate_stat_changes(
        &self,
        user_health_profile: UserHealthProfile,
        hr_data: Vec<HeartRateData>,
        workout_type: WorkoutType
    ) -> Result<WorkoutStats, std::io::Error> {
        self.scoring_method.calculate_stats(user_health_profile, hr_data, workout_type).await
    }
}