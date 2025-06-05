use sqlx::PgPool;
use uuid::Uuid;
use crate::league::schedule::ScheduleService;
use crate::league::games::GameService;
use crate::league::seasons::SeasonService;
use crate::league::countdown::CountdownService;
use crate::league::validation::LeagueValidator;
use crate::league::standings::StandingsService;
use crate::models::league::*;

/// Main league service that orchestrates all league-related operations
pub struct LeagueService {
    _pool: PgPool,
    schedule: ScheduleService,
    standings: StandingsService,
    games: GameService,
    seasons: SeasonService,
    countdown: CountdownService,
    validator: LeagueValidator,
}

impl LeagueService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            _pool: pool.clone(),
            schedule: ScheduleService::new(pool.clone()),
            standings: StandingsService::new(pool.clone()),
            games: GameService::new(pool.clone()),
            seasons: SeasonService::new(pool.clone()),
            countdown: CountdownService::new(),
            validator: LeagueValidator::new(),
        }
    }

    /// Create a new league season (orchestrates multiple services)
    pub async fn create_season(
        &self,
        request: CreateSeasonRequest,
    ) -> Result<LeagueScheduleResponse, sqlx::Error> {
        // Validate the request
        self.validator.validate_create_season_request(&request)?;

        // Create season through season service
        let season = self.seasons.create_season(request.clone()).await?;

        // Initialize standings through standings service
        self.standings.initialize_for_season(season.id, &request.team_ids).await?;

        // Return complete schedule
        self.schedule.get_season_schedule(season.id).await
    }

    /// Get countdown information
    pub async fn get_countdown_info(&self, season_id: Option<Uuid>) -> Result<NextGameInfo, sqlx::Error> {
        let active_season = match season_id {
            Some(id) => self.seasons.get_season(id).await?,
            None => self.seasons.get_active_season().await?,
        };

        match active_season {
            Some(season) => {
                let countdown_seconds = self.countdown.seconds_until_next_game();
                let next_game = self.games.get_next_game(season.id).await?;
                let games_this_week = self.games.get_games_this_week(season.id).await?;
                let week_number = next_game.as_ref().map(|g| g.game.week_number);

                Ok(NextGameInfo {
                    next_game,
                    countdown_seconds,
                    week_number,
                    games_this_week,
                })
            }
            None => {
                // No active season
                Ok(NextGameInfo {
                    next_game: None,
                    countdown_seconds: self.countdown.seconds_until_next_game(),
                    week_number: None,
                    games_this_week: vec![],
                })
            }
        }
    }

    /// Get league standings
    pub async fn get_standings(&self, season_id: Uuid) -> Result<LeagueStandingsResponse, sqlx::Error> {
        self.standings.get_league_standings(season_id).await
    }

    /// Update game result
    pub async fn update_game_result(
        &self,
        game_id: Uuid,
        home_score: i32,
        away_score: i32,
    ) -> Result<(), sqlx::Error> {
        // Validate scores
        self.validator.validate_game_scores(home_score, away_score)?;

        // Update game result
        let game = self.games.update_result(game_id, home_score, away_score).await?;

        // Update standings
        self.standings.update_after_game_result(&game, home_score, away_score).await?;

        Ok(())
    }

    /// Get schedule for a season
    pub async fn get_schedule(&self, season_id: Uuid) -> Result<LeagueScheduleResponse, sqlx::Error> {
        self.schedule.get_season_schedule(season_id).await
    }

    /// Start new season with current teams
    pub async fn start_new_season_with_teams(
        &self,
        team_ids: Vec<Uuid>,
        season_name: Option<String>,
    ) -> Result<LeagueScheduleResponse, sqlx::Error> {
        let season_name = season_name.unwrap_or_else(|| {
            format!("Fantasy Island League {}", chrono::Utc::now().format("%Y"))
        });

        let start_date = self.countdown.get_next_game_time();

        let request = CreateSeasonRequest {
            name: season_name,
            start_date,
            team_ids,
        };

        self.create_season(request).await
    }
}