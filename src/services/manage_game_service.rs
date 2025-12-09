use sqlx::PgPool;
use uuid::Uuid;

use crate::models::league::LeagueGame; // Removed unused import: GameStatus
use crate::db::game_queries::GameQueries;

/// Service for managing games in a season
pub struct ManageGameService {
    game_queries: GameQueries,
}

impl ManageGameService {
    pub fn new(pool: PgPool) -> Self {
        let game_queries = GameQueries::new(pool);
        Self { game_queries }
    }

    /// Start games that should be in progress (current time is within their week window)
    async fn start_due_games(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        tracing::info!("🔍 [GAME_SERVICE] Querying for games ready to start...");
        let games_to_start = self.game_queries.get_games_ready_to_start().await?;

        tracing::info!("🔍 [GAME_SERVICE] Found {} games ready to start", games_to_start.len());
        for game in &games_to_start {
            tracing::info!("   📅 Game {} - start_time: {:?}, status: {:?}",
                game.id, game.game_start_time, game.status);
        }

        let mut started_game_ids = Vec::new();

        for game in games_to_start {
            // Start the game (updates status to 'in_progress' and sets game_start_time)
            tracing::info!("▶️  [GAME_SERVICE] Starting game {}", game.id);
            self.game_queries.start_game(game.id).await?;

            started_game_ids.push(game.id);
            tracing::info!("✅ [GAME_SERVICE] Started game {} with live scoring", game.id);
        }

        if !started_game_ids.is_empty() {
            tracing::info!("✅ [GAME_SERVICE] Started {} games total", started_game_ids.len());
        } else {
            tracing::info!("ℹ️  [GAME_SERVICE] No games to start at this time");
        }

        Ok(started_game_ids)
    }

    /// Finish games where the game has ended
    async fn finish_completed_games(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        tracing::info!("🔍 [GAME_SERVICE] Querying for games that have completed...");
        let games_to_finish = self.game_queries.get_completed_games().await?;

        tracing::info!("🔍 [GAME_SERVICE] Found {} games to finish", games_to_finish.len());
        for game in &games_to_finish {
            tracing::info!("   📅 Game {} - end_time: {:?}, status: {:?}, home_score: {}, away_score: {}",
                game.id, game.game_end_time, game.status, game.home_score, game.away_score);
        }

        let mut finished_game_ids = Vec::new();

        for game in games_to_finish {
            // Finish the game (updates status to 'finished' and sets game_end_time)
            tracing::info!("🏁 [GAME_SERVICE] Finishing game {}", game.id);
            self.game_queries.finish_game(game.id).await?;

            finished_game_ids.push(game.id);
            tracing::info!("✅ [GAME_SERVICE] Finished game {} - ready for final evaluation", game.id);
        }

        if !finished_game_ids.is_empty() {
            tracing::info!("✅ [GAME_SERVICE] Finished {} games total", finished_game_ids.len());
        } else {
            tracing::info!("ℹ️  [GAME_SERVICE] No games to finish at this time");
        }

        Ok(finished_game_ids)
    }


    /// Get all games that are currently active
    pub async fn get_active_games(&self) -> Result<Vec<LeagueGame>, sqlx::Error> {
        self.game_queries.get_active_games().await
    }

    /// Get all games that are ready to start
    async fn get_games_ready_to_start(&self) -> Result<Vec<LeagueGame>, sqlx::Error> {
        self.game_queries.get_games_ready_to_start().await
    }

    /// Run the game management cycle (start new games, finish completed ones)
    pub async fn run_game_cycle(&self) -> Result<(Vec<Uuid>, Vec<Uuid>, Vec<Uuid>, Vec<Uuid>), sqlx::Error> {
        tracing::info!("🔄 [GAME_SERVICE] Starting game cycle run");

        let games_ready_to_start: Vec<Uuid> = self.get_games_ready_to_start().await?.iter().map(|game| game.id).collect();
        let live_games: Vec<Uuid> = self.get_active_games().await?.iter().map(|game| game.id).collect();

        tracing::info!("🔄 [GAME_SERVICE] Current state: {} games ready to start, {} games live",
            games_ready_to_start.len(), live_games.len());

        let started_games = self.start_due_games().await?;
        let finished_games = self.finish_completed_games().await?;

        tracing::info!("🔄 [GAME_SERVICE] Cycle complete: {} started, {} finished",
            started_games.len(), finished_games.len());

        Ok((games_ready_to_start, live_games, started_games, finished_games))
    }
}