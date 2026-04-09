use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

pub mod analytics;
pub mod anomaly_sweep;
pub mod consistency;
pub mod personal_bests;

pub use self::analytics::{DailyStatsJob, MonthlyStatsJob, WeeklyStatsJob};
pub use self::anomaly_sweep::AnomalySweepJob;
pub use self::consistency::ConsistencyValidationJob;
pub use self::personal_bests::PersonalBestsJob;

/// Each job decides internally whether enough time has elapsed before doing
/// real work.  The scheduler calls `maybe_run` on every 60-second tick.
#[async_trait]
pub trait Job: Send + Sync {
    fn name(&self) -> &'static str;
    async fn maybe_run(&self, pool: &PgPool) -> Result<()>;
}

pub struct JobRegistry {
    jobs: Vec<Box<dyn Job>>,
}

impl JobRegistry {
    pub fn iter(&self) -> impl Iterator<Item = &dyn Job> {
        self.jobs.iter().map(|b| b.as_ref())
    }
}

pub fn register() -> JobRegistry {
    JobRegistry {
        jobs: vec![
            Box::new(DailyStatsJob::new()),
            Box::new(WeeklyStatsJob::new()),
            Box::new(MonthlyStatsJob::new()),
            Box::new(PersonalBestsJob::new()),
            Box::new(ConsistencyValidationJob::new()),
            Box::new(AnomalySweepJob::new()),
        ],
    }
}
