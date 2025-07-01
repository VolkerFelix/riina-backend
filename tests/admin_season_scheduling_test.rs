use reqwest::Client;
use serde_json::json;
use chrono::{Weekday, NaiveTime, Utc, Duration};

mod common;
use common::utils::{spawn_app, make_authenticated_request, get_next_date};
use common::admin_helpers::{create_admin_user_and_login, create_league_season_with_schedule, create_teams_for_test};

#[tokio::test]
async fn test_season_creation_with_dynamic_scheduling() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🎯 Testing Dynamic Season Scheduling");
    
    // Step 1: Create admin user
    let admin_user = create_admin_user_and_login(&app.address).await;
    println!("✅ Created admin user");

    // Step 2: Create a league
    let league_request = json!({
        "name": "Dynamic Scheduling Test League",
        "description": "Testing dynamic season scheduling",
        "max_teams": 4
    });
    
    let league_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", &app.address),
        &admin_user.token,
        Some(league_request),
    ).await;
    
    assert_eq!(league_response.status(), 201);
    let league_data: serde_json::Value = league_response.json().await.unwrap();
    let league_id = league_data["data"]["id"].as_str().unwrap();
    
    // Step 3: Create teams for the league
    let team_ids = create_teams_for_test(&app.address, &admin_user.token, 4).await;
    
    // Assign teams to league
    for team_id in &team_ids {
        let assign_request = json!({"team_id": team_id});
        let assign_response = make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", &app.address, league_id),
            &admin_user.token,
            Some(assign_request),
        ).await;
        assert_eq!(assign_response.status(), 201);
    }
    
    println!("✅ Created and assigned 4 teams to league");

    // Step 4: Test default scheduling (Saturday 10 PM UTC)
    let start_date = get_next_date(Weekday::Mon, NaiveTime::from_hms_opt(9, 0, 0).unwrap());
    
    let season_id_default = create_league_season_with_schedule(
        &app.address,
        &admin_user.token,
        league_id,
        "Default Schedule Season",
        &start_date.to_rfc3339(),
        None, // Use default cron
        None, // Use default timezone
        None, // Use default auto_evaluation_enabled
    ).await;
    
    // Fetch season details to verify defaults
    let season_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues/{}/seasons/{}", &app.address, league_id, season_id_default),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(season_response.status(), 200);
    let season_data: serde_json::Value = season_response.json().await.unwrap();
    
    // Verify default values
    assert_eq!(season_data["data"]["evaluation_cron"].as_str().unwrap(), "0 0 22 * * SAT");
    assert_eq!(season_data["data"]["evaluation_timezone"].as_str().unwrap(), "UTC");
    assert_eq!(season_data["data"]["auto_evaluation_enabled"].as_bool().unwrap(), true);
    
    println!("✅ Created season with default schedule (Saturday 10 PM UTC)");

    // Step 5: Test custom scheduling (Tuesday 8 AM UTC)
    let season_id_custom = create_league_season_with_schedule(
        &app.address,
        &admin_user.token,
        league_id,
        "Custom Schedule Season",
        &(start_date + Duration::days(30)).to_rfc3339(),
        Some("0 0 8 * * TUE"), // Tuesday 8 AM
        Some("UTC"),
        Some(true),
    ).await;
    
    // Fetch season details to verify custom values
    let custom_season_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues/{}/seasons/{}", &app.address, league_id, season_id_custom),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(custom_season_response.status(), 200);
    let custom_season_data: serde_json::Value = custom_season_response.json().await.unwrap();
    
    // Verify custom values
    assert_eq!(custom_season_data["data"]["evaluation_cron"].as_str().unwrap(), "0 0 8 * * TUE");
    assert_eq!(custom_season_data["data"]["evaluation_timezone"].as_str().unwrap(), "UTC");
    assert_eq!(custom_season_data["data"]["auto_evaluation_enabled"].as_bool().unwrap(), true);
    
    println!("✅ Created season with custom schedule (Tuesday 8 AM UTC)");

    // Step 6: Test disabled auto-evaluation
    let season_id_disabled = create_league_season_with_schedule(
        &app.address,
        &admin_user.token,
        league_id,
        "Disabled Auto-Evaluation Season",
        &(start_date + Duration::days(60)).to_rfc3339(),
        Some("0 0 15 * * WED"), // Wednesday 3 PM
        Some("Europe/London"),
        Some(false), // Disable auto-evaluation
    ).await;
    
    // Fetch season details to verify disabled auto-evaluation
    let disabled_season_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues/{}/seasons/{}", &app.address, league_id, season_id_disabled),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(disabled_season_response.status(), 200);
    let disabled_season_data: serde_json::Value = disabled_season_response.json().await.unwrap();
    
    // Verify disabled auto-evaluation
    assert_eq!(disabled_season_data["data"]["evaluation_cron"].as_str().unwrap(), "0 0 15 * * WED");
    assert_eq!(disabled_season_data["data"]["evaluation_timezone"].as_str().unwrap(), "Europe/London");
    assert_eq!(disabled_season_data["data"]["auto_evaluation_enabled"].as_bool().unwrap(), false);
    
    println!("✅ Created season with disabled auto-evaluation");

    // Step 7: Verify end date is calculated based on team count (4 teams = 6 weeks)
    let expected_end_date = start_date + Duration::weeks(6);
    let actual_end_date = chrono::DateTime::parse_from_rfc3339(
        season_data["data"]["end_date"].as_str().unwrap()
    ).unwrap();
    
    // Check that the dates are on the same day (allowing for time differences)
    assert_eq!(
        actual_end_date.date_naive(),
        expected_end_date.date_naive(),
        "End date should be 6 weeks after start date for 4 teams"
    );
    
    println!("✅ Verified end date calculation (6 weeks for 4 teams)");

    // Step 8: Test invalid cron expression
    let invalid_cron_request = json!({
        "name": "Invalid Cron Season",
        "start_date": (start_date + Duration::days(90)).to_rfc3339(),
        "evaluation_cron": "invalid cron expression",
        "evaluation_timezone": "UTC",
        "auto_evaluation_enabled": true
    });
    
    let invalid_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues/{}/seasons", &app.address, league_id),
        &admin_user.token,
        Some(invalid_cron_request),
    ).await;
    
    // Should still create the season but log an error about scheduling
    assert_eq!(invalid_response.status(), 201);
    println!("✅ Season created even with invalid cron (scheduler error logged)");

    // Step 9: Delete a season and verify scheduler cleanup
    let delete_response = make_authenticated_request(
        &client,
        reqwest::Method::DELETE,
        &format!("{}/admin/leagues/{}/seasons/{}", &app.address, league_id, season_id_custom),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(delete_response.status(), 204);
    println!("✅ Deleted season (scheduler cleanup should occur)");

    // Step 10: List all seasons to verify they were created
    let list_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues/{}/seasons", &app.address, league_id),
        &admin_user.token,
        None,
    ).await;
    
    assert_eq!(list_response.status(), 200);
    let list_data: serde_json::Value = list_response.json().await.unwrap();
    let seasons = list_data["data"].as_array().unwrap();
    
    // Should have 3 seasons (deleted one)
    assert_eq!(seasons.len(), 3);
    
    println!("✅ Listed seasons successfully");
    println!("🎉 Dynamic season scheduling test completed successfully!");
}

#[tokio::test]
async fn test_season_scheduling_edge_cases() {
    let app = spawn_app().await;
    let client = Client::new();
    
    println!("🎯 Testing Season Scheduling Edge Cases");
    
    // Create admin and league
    let admin_user = create_admin_user_and_login(&app.address).await;
    
    let league_request = json!({
        "name": "Edge Case Test League",
        "description": "Testing edge cases",
        "max_teams": 2
    });
    
    let league_response = make_authenticated_request(
        &client,
        reqwest::Method::POST,
        &format!("{}/admin/leagues", &app.address),
        &admin_user.token,
        Some(league_request),
    ).await;
    
    let league_data: serde_json::Value = league_response.json().await.unwrap();
    let league_id = league_data["data"]["id"].as_str().unwrap();
    
    // Create minimal teams
    let team_ids = create_teams_for_test(&app.address, &admin_user.token, 2).await;
    for team_id in &team_ids {
        let assign_request = json!({"team_id": team_id});
        make_authenticated_request(
            &client,
            reqwest::Method::POST,
            &format!("{}/admin/leagues/{}/teams", &app.address, league_id),
            &admin_user.token,
            Some(assign_request),
        ).await;
    }

    // Test 1: Very frequent evaluation (every hour)
    let hourly_cron = "0 0 * * * *"; // Every hour
    let season_id_hourly = create_league_season_with_schedule(
        &app.address,
        &admin_user.token,
        league_id,
        "Hourly Evaluation Season",
        &get_next_date(Weekday::Wed, NaiveTime::from_hms_opt(10, 0, 0).unwrap()).to_rfc3339(),
        Some(hourly_cron),
        Some("UTC"),
        Some(true),
    ).await;
    
    println!("✅ Created season with hourly evaluation schedule");

    // Test 2: Different timezone
    let season_id_timezone = create_league_season_with_schedule(
        &app.address,
        &admin_user.token,
        league_id,
        "Tokyo Timezone Season",
        &get_next_date(Weekday::Thu, NaiveTime::from_hms_opt(12, 0, 0).unwrap()).to_rfc3339(),
        Some("0 0 21 * * MON"), // Monday 9 PM
        Some("Asia/Tokyo"),
        Some(true),
    ).await;
    
    println!("✅ Created season with Asia/Tokyo timezone");

    // Test 3: Complex cron expression (first Monday of month at 2 PM)
    let complex_cron = "0 0 14 1-7 * MON"; // First Monday of month at 2 PM
    let season_id_complex = create_league_season_with_schedule(
        &app.address,
        &admin_user.token,
        league_id,
        "Monthly Evaluation Season",
        &get_next_date(Weekday::Fri, NaiveTime::from_hms_opt(15, 0, 0).unwrap()).to_rfc3339(),
        Some(complex_cron),
        Some("UTC"),
        Some(true),
    ).await;
    
    println!("✅ Created season with complex monthly schedule");

    // Verify all seasons were created
    let list_response = make_authenticated_request(
        &client,
        reqwest::Method::GET,
        &format!("{}/admin/leagues/{}/seasons", &app.address, league_id),
        &admin_user.token,
        None,
    ).await;
    
    let list_data: serde_json::Value = list_response.json().await.unwrap();
    let seasons = list_data["data"].as_array().unwrap();
    assert_eq!(seasons.len(), 3);
    
    println!("🎉 Season scheduling edge cases test completed successfully!");
}