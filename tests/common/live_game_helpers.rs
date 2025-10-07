    use reqwest::Client;
    use serde_json::json;
    use chrono::{Utc, Duration, Weekday, NaiveTime, DateTime};
    use uuid::Uuid;
    use serde::{Serialize, Deserialize};
    use std::sync::Arc;

    use crate::common::utils::*;
    use crate::common::admin_helpers::*;
    use crate::common::workout_data_helpers::*;

    pub async fn test_live_scoring_history_api(
        test_app: &TestApp, 
        client: &Client, 
        token: &str, 
        game_id: Uuid,
        home_user: &UserRegLoginResponse,
        away_user_1: &UserRegLoginResponse,
        away_user_2: &UserRegLoginResponse
    ) {
        println!("🧪 Testing live scoring history API endpoint...");
        
        // Call the live game API endpoint that should include scoring events
        let url = format!("{}/league/games/{}/live", test_app.address, game_id);
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .expect("Failed to call live game API");

        assert!(response.status().is_success(), "Live game API should return success");
        
        let response_data: serde_json::Value = response.json().await.expect("Failed to parse response");
        assert_eq!(response_data["success"], true, "API response should indicate success");
        
        // Verify the response structure matches what the frontend expects
        let data = response_data["data"].as_object().expect("Data should be an object");
        
        // Check required fields
        assert!(data.contains_key("game_id"), "Should contain game_id");
        assert!(data.contains_key("home_team_name"), "Should contain home_team_name");
        assert!(data.contains_key("away_team_name"), "Should contain away_team_name");
        assert!(data.contains_key("home_score"), "Should contain home_score");
        assert!(data.contains_key("away_score"), "Should contain away_score");
        assert!(data.contains_key("status"), "Should contain status");
        
        // Most importantly, check scoring_events
        assert!(data.contains_key("scoring_events"), "Should contain scoring_events array");
        
        let scoring_events = data["scoring_events"].as_array().expect("scoring_events should be an array");
        assert!(!scoring_events.is_empty(), "Should have scoring events from the test uploads");
        
        // Verify scoring event structure matches frontend expectations
        for event in scoring_events {
            let event_obj = event.as_object().expect("Each scoring event should be an object");
            
            // Check all required fields for frontend parsing
            assert!(event_obj.contains_key("id"), "Event should have id");
            assert!(event_obj.contains_key("user_id"), "Event should have user_id");
            assert!(event_obj.contains_key("username"), "Event should have username");
            assert!(event_obj.contains_key("team_id"), "Event should have team_id");
            assert!(event_obj.contains_key("team_side"), "Event should have team_side");
            assert!(event_obj.contains_key("score_points"), "Event should have score_points");
            assert!(event_obj.contains_key("description"), "Event should have description");
            assert!(event_obj.contains_key("occurred_at"), "Event should have occurred_at timestamp");
            
            // Verify team_side is valid
            let team_side = event_obj["team_side"].as_str().expect("team_side should be a string");
            assert!(team_side == "home" || team_side == "away", "team_side should be 'home' or 'away'");
            
            // Verify score_points is positive (since our test uploads should generate points)
            let score_points = event_obj["score_points"].as_i64().expect("score_points should be a number");
            assert!(score_points > 0, "Test uploads should generate positive points");
            
            // Verify the user_id matches one of our test users
            let event_user_id = event_obj["user_id"].as_str().expect("user_id should be a string");
            let event_user_uuid = Uuid::parse_str(event_user_id).expect("user_id should be valid UUID");
            assert!(
                event_user_uuid == home_user.user_id || event_user_uuid == away_user_1.user_id || event_user_uuid == away_user_2.user_id,
                "Event should be from one of our test users"
            );

            // Verify workout_details are present and properly structured
            assert!(event_obj.contains_key("workout_details"), "Event should have workout_details");
            
            if let Some(workout_details) = event_obj["workout_details"].as_object() {
                // Check that essential workout detail fields are present
                assert!(workout_details.contains_key("id"), "workout_details should have id");
                assert!(workout_details.contains_key("workout_date"), "workout_details should have workout_date");
                assert!(workout_details.contains_key("workout_start"), "workout_details should have workout_start");
                assert!(workout_details.contains_key("workout_end"), "workout_details should have workout_end");
                assert!(workout_details.contains_key("stamina_gained"), "workout_details should have stamina_gained");
                assert!(workout_details.contains_key("strength_gained"), "workout_details should have strength_gained");
                
                // For our test data that includes heart rate, verify those fields are present
                assert!(workout_details.contains_key("duration_minutes"), "workout_details should have duration_minutes");
                assert!(workout_details.contains_key("avg_heart_rate"), "workout_details should have avg_heart_rate");
                assert!(workout_details.contains_key("max_heart_rate"), "workout_details should have max_heart_rate");
                assert!(workout_details.contains_key("heart_rate_zones"), "workout_details should have heart_rate_zones");
                
                // Verify that the workout_details have actual values (not all null)
                // Our test workouts should have duration since they have start/end times
                if let (Some(start), Some(end)) = (workout_details["workout_start"].as_str(), workout_details["workout_end"].as_str()) {
                    let workout_start = chrono::DateTime::parse_from_rfc3339(start).expect("Should parse workout_start");
                    let workout_end = chrono::DateTime::parse_from_rfc3339(end).expect("Should parse workout_end");
                    let expected_duration_minutes = (workout_end - workout_start).num_minutes();
                    
                    if expected_duration_minutes > 0 {
                        // The database should have the calculated duration, not null
                        assert!(
                            !workout_details["duration_minutes"].is_null(),
                            "workout_details.duration_minutes should not be null when workout has start/end times"
                        );
                        
                        let actual_duration = workout_details["duration_minutes"].as_i64()
                            .expect("duration_minutes should be a number, not null");
                        assert!(
                            actual_duration > 0,
                            "workout_details should have calculated duration_minutes > 0, got: {}",
                            actual_duration
                        );
                        
                        // Verify the calculated duration is reasonable (within 1 minute of expected)
                        let duration_diff = (actual_duration - expected_duration_minutes).abs();
                        assert!(
                            duration_diff <= 1,
                            "Calculated duration {} should be close to expected {}", 
                            actual_duration, expected_duration_minutes
                        );
                    }
                }
                
                // Also verify heart rate data is properly calculated if present
                if workout_details.contains_key("avg_heart_rate") && !workout_details["avg_heart_rate"].is_null() {
                    let avg_hr = workout_details["avg_heart_rate"].as_f64()
                        .expect("avg_heart_rate should be a number if not null");
                    assert!(avg_hr > 0.0, "avg_heart_rate should be positive, got: {}", avg_hr);
                    assert!(avg_hr < 300.0, "avg_heart_rate should be reasonable, got: {}", avg_hr);
                }
                
                // Verify heart rate zones are properly calculated and stored
                // Our test workouts include heart rate data, so zones should be calculated
                assert!(
                    !workout_details["heart_rate_zones"].is_null(),
                    "heart_rate_zones should not be null when workout has heart rate data"
                );
                
                if let Some(zones) = workout_details["heart_rate_zones"].as_array() {
                    assert!(!zones.is_empty(), "heart_rate_zones should contain zone data when heart rate is present");
                    
                    // Verify zone structure
                    for zone in zones {
                        let zone_obj = zone.as_object().expect("Each zone should be an object");
                        assert!(zone_obj.contains_key("zone"), "Zone should have 'zone' field");
                        assert!(zone_obj.contains_key("minutes"), "Zone should have 'minutes' field");
                        assert!(zone_obj.contains_key("stamina_gained"), "Zone should have 'stamina_gained' field");
                        assert!(zone_obj.contains_key("strength_gained"), "Zone should have 'strength_gained' field");
                        
                        // Verify zone has reasonable values
                        let zone_name = zone_obj["zone"].as_str().expect("zone should be a string");
                        assert!(
                            ["Zone1", "Zone2", "Zone3", "Zone4", "Zone5"].contains(&zone_name),
                            "Zone name should be valid, got: {}", zone_name
                        );
                        
                        let minutes = zone_obj["minutes"].as_f64().expect("minutes should be a number");
                        assert!(minutes >= 0.0, "Zone minutes should be non-negative, got: {}", minutes);
                    }
                    
                    // Verify that total zone minutes roughly equals workout duration
                    let total_zone_minutes: f64 = zones.iter()
                        .map(|z| z["minutes"].as_f64().unwrap_or(0.0))
                        .sum();
                    
                    if let Some(duration) = workout_details["duration_minutes"].as_i64() {
                        let duration_diff = (total_zone_minutes - duration as f64).abs();
                        assert!(
                            duration_diff < 2.0,
                            "Total zone minutes {} should roughly equal workout duration {}",
                            total_zone_minutes, duration
                        );
                    }
                } else {
                    panic!("heart_rate_zones should be an array when heart rate data is present");
                }
                
                println!("✅ Workout details verified for event {}", event_obj["id"].as_str().unwrap_or("unknown"));
            } else {
                panic!("workout_details should be an object, not null");
            }
        }
        
        // Check that events are ordered by most recent first (as expected by frontend)
        if scoring_events.len() > 1 {
            for i in 0..scoring_events.len()-1 {
                let current_time = scoring_events[i]["occurred_at"].as_str().expect("Should have timestamp");
                let next_time = scoring_events[i+1]["occurred_at"].as_str().expect("Should have timestamp");
                
                let current_dt = chrono::DateTime::parse_from_rfc3339(current_time).expect("Should parse timestamp");
                let next_dt = chrono::DateTime::parse_from_rfc3339(next_time).expect("Should parse timestamp");
                
                assert!(current_dt >= next_dt, "Events should be ordered by most recent first");
            }
        }
        
        println!("✅ Live scoring history API test passed!");
        println!("   - Found {} scoring events", scoring_events.len());
        println!("   - All required fields present and valid");
        println!("   - Events properly ordered by timestamp");
    }

    // Helper functions

    pub struct LiveGameEnvironmentResult {
        pub admin_session: UserRegLoginResponse,
        pub league_id: String,
        pub home_team_id: String,
        pub away_team_id: String,
        pub home_user: UserRegLoginResponse,
        pub away_user_1: UserRegLoginResponse,
        pub away_user_2: UserRegLoginResponse,
        pub season_id: String,
        pub first_game_id: Uuid,
    }

    pub async fn setup_live_game_environment(
        test_app: &TestApp, 
    ) -> LiveGameEnvironmentResult {
        let client = Client::new();
        let admin_session = create_admin_user_and_login(&test_app.address).await;
        // Create league
        let league_id = create_league(&test_app.address, &admin_session.token, 2).await;

        // Create teams
        let team_ids = create_teams_for_test(&test_app.address, &admin_session.token, 2).await;
        let team1_id = team_ids[0].clone();
        let team2_id = team_ids[1].clone();

        // Add teams to league BEFORE creating the season so games are auto-generated
        add_team_to_league(&test_app.address, &admin_session.token, &league_id, &team1_id).await;
        add_team_to_league(&test_app.address, &admin_session.token, &league_id, &team2_id).await;

        let start_date = get_next_date(Weekday::Sat, NaiveTime::from_hms_opt(22, 0, 0).unwrap());
        let season_id = create_league_season(
            &test_app.address,
            &admin_session.token,
            &league_id,
            &format!("Test Season - {}", start_date.to_rfc3339()),
            &start_date.to_rfc3339()
        ).await;
        
        // Get the auto-generated game and find out which team is actually home vs away
        let (first_game_id, actual_home_team_id, actual_away_team_id) = get_first_game_for_teams(
            &test_app, 
            Uuid::parse_str(&season_id).unwrap(), 
            Uuid::parse_str(&team1_id).unwrap(), 
            Uuid::parse_str(&team2_id).unwrap()
        ).await;
        
        // Assign team IDs based on actual game configuration
        let home_team_id = actual_home_team_id.to_string();
        let away_team_id = actual_away_team_id.to_string();

        // Create additional users with health profiles and add them to the correct teams
        let home_user = create_test_user_and_login(&test_app.address).await;
        create_health_profile_for_user(&client, &test_app.address, &home_user).await.unwrap();
        let away_user_1 = create_test_user_and_login(&test_app.address).await;
        create_health_profile_for_user(&client, &test_app.address, &away_user_1).await.unwrap();
        let away_user_2 = create_test_user_and_login(&test_app.address).await;
        create_health_profile_for_user(&client, &test_app.address, &away_user_2).await.unwrap();

        // Add users to teams based on actual home/away assignments
        add_user_to_team(&test_app.address, &admin_session.token, &home_team_id, home_user.user_id).await;
        add_user_to_team(&test_app.address, &admin_session.token, &away_team_id, away_user_1.user_id).await;
        add_user_to_team(&test_app.address, &admin_session.token, &away_team_id, away_user_2.user_id).await;

        LiveGameEnvironmentResult {
            admin_session,
            league_id,
            home_team_id,
            away_team_id,
            home_user,
            away_user_1,
            away_user_2,
            season_id,
            first_game_id,
        }
    }

    pub async fn get_first_game_for_teams(test_app: &TestApp, season_id: Uuid, team1_id: Uuid, team2_id: Uuid) -> (Uuid, Uuid, Uuid) {
        // Get the auto-generated game between these teams and return actual home/away assignments
        let game = sqlx::query!(
            r#"
            SELECT id, home_team_id, away_team_id
            FROM games 
            WHERE season_id = $1 
            AND ((home_team_id = $2 AND away_team_id = $3) OR (home_team_id = $3 AND away_team_id = $2))
            ORDER BY week_number
            LIMIT 1
            "#,
            season_id,
            team1_id,
            team2_id
        )
        .fetch_one(&test_app.db_pool)
        .await
        .expect("Failed to find auto-generated game");
        
        (game.id, game.home_team_id, game.away_team_id)
    }

    pub async fn create_test_game(test_app: &TestApp, home_team_id: Uuid, away_team_id: Uuid, season_id: Uuid) -> Uuid {
        let game_start = Utc::now();
        let game_end = game_start + Duration::hours(2);

        let game_id = Uuid::new_v4();
        
        // Insert game directly into database for testing
        sqlx::query!(
            r#"
            INSERT INTO games (id, home_team_id, away_team_id, season_id, week_number, game_start_time, game_end_time, status)
            VALUES ($1, $2, $3, $4, 1, $5, $6, 'in_progress')
            "#,
            game_id,
            home_team_id,
            away_team_id,
            season_id,
            game_start,
            game_end
        )
        .execute(&test_app.db_pool)
        .await
        .expect("Failed to insert test game");

        game_id
    }

    pub async fn update_game_to_short_duration(test_app: &TestApp, game_id: Uuid) {
        let game_start = Utc::now();
        let game_end = game_start + Duration::minutes(1); // Very short game for testing
        
        sqlx::query!(
            r#"
            UPDATE games 
            SET game_start_time = $1, game_end_time = $2
            WHERE id = $3
            "#,
            game_start,
            game_end,
            game_id
        )
        .execute(&test_app.db_pool)
        .await
        .expect("Failed to update game duration");
    }

    pub async fn update_game_times_to_now(test_app: &TestApp, game_id: Uuid) {
        let now = Utc::now();
        let game_end = now + Duration::hours(2);
        
        sqlx::query!(
            r#"
            UPDATE games 
            SET game_start_time = $1, game_end_time = $2
            WHERE id = $3
            "#,
            now,
            game_end,
            game_id
        )
        .execute(&test_app.db_pool)
        .await
        .expect("Failed to update game times to current");
    }

    pub async fn start_test_game(test_app: &TestApp, game_id: Uuid) {
        let now = Utc::now();
        let game_end = now + Duration::hours(2);
        
        sqlx::query!(
            r#"
            UPDATE games 
            SET status = 'in_progress', 
                game_start_time = $1,
                game_end_time = $2
            WHERE id = $3
            "#,
            now,
            game_end,
            game_id
        )
        .execute(&test_app.db_pool)
        .await
        .expect("Failed to start test game");
    }

    pub async fn initialize_live_game(test_app: &TestApp, game_id: Uuid, _redis_client: Arc<redis::Client>) -> LiveGameRow {
        // In the consolidated architecture, games don't need separate initialization
        // Just start the game directly and return its state
        start_test_game(test_app, game_id).await;
        get_live_game_state(test_app, game_id).await
    }

    pub async fn get_season_id_for_game(test_app: &TestApp, game_id: Uuid) -> Uuid {
        let row = sqlx::query!(
            "SELECT season_id FROM games WHERE id = $1",
            game_id
        )
        .fetch_one(&test_app.db_pool)
        .await
        .expect("Failed to get season ID for game");
        
        row.season_id
    }

    pub async fn get_live_games_via_api(test_app: &TestApp, client: &Client, token: &str, season_id: Option<Uuid>) -> Vec<serde_json::Value> {
        let mut url = format!("{}/league/games/live-active", test_app.address);
        if let Some(sid) = season_id {
            url = format!("{}?season_id={}", url, sid);
        }

        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .expect("Failed to get live games");

        assert!(response.status().is_success(), "Failed to get live games from API");
        
        let data: serde_json::Value = response.json().await.expect("Failed to parse response");
        assert_eq!(data["success"], true);
        
        data["data"].as_array().unwrap().clone()
    }

    pub async fn get_live_game_state(test_app: &TestApp, game_id: Uuid) -> LiveGameRow {
        // Query the consolidated games table with team names
        let row = sqlx::query!(
            r#"
            SELECT 
                g.id, g.home_team_id, g.away_team_id,
                ht.team_name as home_team_name, at.team_name as away_team_name,
                g.home_score, g.away_score, g.status,
                g.game_start_time, g.game_end_time, g.last_score_time, 
                g.last_scorer_id, g.last_scorer_name, g.last_scorer_team,
                g.created_at, g.updated_at
            FROM games g
            JOIN teams ht ON g.home_team_id = ht.id
            JOIN teams at ON g.away_team_id = at.id
            WHERE g.id = $1
            "#,
            game_id
        )
        .fetch_one(&test_app.db_pool)
        .await
        .expect("Failed to get live game state");

        LiveGameRow {
            id: row.id,
            game_id: row.id, // In consolidated table, id IS the game_id
            home_team_name: row.home_team_name,
            away_team_name: row.away_team_name,
            home_score: row.home_score,
            away_score: row.away_score,
            game_start_time: row.game_start_time,
            game_end_time: row.game_end_time,
            last_score_time: row.last_score_time,
            last_scorer_id: row.last_scorer_id,
            last_scorer_name: row.last_scorer_name,
            last_scorer_team: row.last_scorer_team
        }
    }

    pub async fn get_player_contributions(test_app: &TestApp, live_game_id: Uuid) -> (Vec<PlayerContribution>, Vec<PlayerContribution>) {
        // Get the live game info first
        let live_game = sqlx::query!(
            "SELECT home_team_id, away_team_id FROM games WHERE id = $1",
            live_game_id
        )
        .fetch_one(&test_app.db_pool)
        .await
        .expect("Failed to get live game info");

        // Get contributions by joining team_members with aggregated score events (same logic as the main system)
        let rows = sqlx::query!(
            r#"
            SELECT 
                tm.user_id,
                u.username,
                tm.team_id,
                t.team_name,
                CASE 
                    WHEN tm.team_id = $2 THEN 'home'
                    WHEN tm.team_id = $3 THEN 'away'
                    ELSE 'unknown'
                END as team_side,
                COALESCE(SUM(lse.power_contribution), 0)::int as current_power,
                COALESCE(SUM(lse.score_points), 0)::int as total_score_contribution,
                COUNT(CASE WHEN lse.id IS NOT NULL THEN 1 END)::int as contribution_count,
                MAX(lse.occurred_at) as last_contribution_time
            FROM team_members tm
            JOIN users u ON tm.user_id = u.id
            JOIN teams t ON tm.team_id = t.id
            LEFT JOIN live_score_events lse ON lse.game_id = $1 AND lse.user_id = tm.user_id
            WHERE tm.status = 'active'
            AND (tm.team_id = $2 OR tm.team_id = $3)
            GROUP BY tm.user_id, u.username, tm.team_id, t.team_name
            ORDER BY total_score_contribution DESC
            "#,
            live_game_id,
            live_game.home_team_id,
            live_game.away_team_id
        )
        .fetch_all(&test_app.db_pool)
        .await
        .expect("Failed to get player contributions");
        
        println!("DEBUG: Found {} team members for live_game_id: {}", rows.len(), live_game_id);
        println!("DEBUG: home_team_id: {}, away_team_id: {}", live_game.home_team_id, live_game.away_team_id);

        let mut home_contributions = Vec::new();
        let mut away_contributions = Vec::new();

        for row in rows {
            let team_side = row.team_side.as_ref().unwrap_or(&"unknown".to_string()).clone();
            let contrib = PlayerContribution {
                user_id: row.user_id,
                username: row.username,
                team_side: team_side.clone(),
                total_score_contribution: row.total_score_contribution.unwrap_or(0),
                contribution_count: row.contribution_count.unwrap_or(0),
                last_contribution_time: row.last_contribution_time,
            };

            if team_side == "home" {
                home_contributions.push(contrib);
            } else {
                away_contributions.push(contrib);
            }
        }

        (home_contributions, away_contributions)
    }

    pub async fn get_recent_score_events(test_app: &TestApp, live_game_id: Uuid) -> Vec<ScoreEvent> {
        let rows = sqlx::query!(
            r#"
            SELECT user_id, team_side, score_points, occurred_at
            FROM live_score_events 
            WHERE game_id = $1
            ORDER BY occurred_at DESC
            "#,
            live_game_id
        )
        .fetch_all(&test_app.db_pool)
        .await
        .expect("Failed to get score events");

        rows.into_iter().map(|row| ScoreEvent {
            user_id: row.user_id,
            team_side: row.team_side,
            score_points: row.score_points,
            occurred_at: row.occurred_at,
        }).collect()
    }

    pub async fn finish_live_game(test_app: &TestApp, game_id: Uuid, _redis_client: Arc<redis::Client>) {
        // In the consolidated architecture, just update the game status to finished
        sqlx::query!(
            "UPDATE games SET status = 'finished' WHERE id = $1",
            game_id
        )
        .execute(&test_app.db_pool)
        .await
        .expect("Failed to finish test game");
    }

    // Test data structures
    #[derive(Debug, Serialize, Deserialize)]
    pub struct LiveGameRow {
        pub id: Uuid,
        pub game_id: Uuid,
        pub home_team_name: String,
        pub away_team_name: String,
        pub home_score: i32,
        pub away_score: i32,
        pub game_start_time: Option<DateTime<Utc>>,
        pub game_end_time: Option<DateTime<Utc>>,
        pub last_score_time: Option<DateTime<Utc>>,
        pub last_scorer_id: Option<Uuid>,
        pub last_scorer_name: Option<String>,
        pub last_scorer_team: Option<String>,
    }

    impl LiveGameRow {
        /// Calculate game progress as percentage (0-100)
        pub fn game_progress(&self) -> f32 {
            let now = Utc::now();
            let game_start_time = match self.game_start_time {
                Some(time) => time,
                // Game hasn't started yet
                None => return 0.0,
            };
            let game_end_time = match self.game_end_time {
                Some(time) => time,
                // Game has ended
                None => {
                    panic!("Game end time is not set");
                }
            };
            
            let total_duration = (game_end_time - game_start_time).num_milliseconds() as f32;
            let elapsed = (now - game_start_time).num_milliseconds() as f32;
            
            (elapsed / total_duration * 100.0).clamp(0.0, 100.0)
        }

        /// Get time remaining in human readable format
        pub fn time_remaining(&self) -> Option<String> {
            let now = Utc::now();
            let game_end_time = match self.game_end_time {
                Some(time) => time,
                // Game has ended
                None => {
                    panic!("Game end time is not set");
                }
            };
            if now >= game_end_time {
                return Some("Final".to_string());
            }

            let remaining = game_end_time - now;
            let hours = remaining.num_hours();
            let minutes = remaining.num_minutes() % 60;

            if hours > 0 {
                Some(format!("{}h {}m", hours, minutes))
            } else if minutes > 0 {
                Some(format!("{}m", minutes))
            } else {
                Some("< 1m".to_string())
            }
        }
    }

    #[derive(Debug)]
    pub struct PlayerContribution {
        pub user_id: Uuid,
        pub username: String,
        pub team_side: String,
        pub total_score_contribution: i32,
        pub contribution_count: i32,
        pub last_contribution_time: Option<chrono::DateTime<Utc>>,
    }

    impl PlayerContribution {
        pub fn is_recently_active(&self) -> bool {
            if let Some(last_contribution) = self.last_contribution_time {
                let thirty_minutes_ago = Utc::now() - Duration::minutes(30);
                last_contribution > thirty_minutes_ago
            } else {
                false
            }
        }
    }

    #[derive(Debug)]
    pub struct ScoreEvent {
        pub user_id: Uuid,
        pub team_side: String,
        pub score_points: f32,
        pub occurred_at: chrono::DateTime<Utc>,
    }