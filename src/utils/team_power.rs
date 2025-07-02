// src/utils/team_power.rs
use sqlx::PgPool;
use uuid::Uuid;
use std::collections::HashMap;
use crate::models::common::PlayerStats;

/// Team member with stats for power calculation
#[derive(Debug)]
pub struct TeamMemberStats {
    pub user_id: Uuid,
    pub stats: PlayerStats,
    pub status: String,
}

/// Calculate team power based on member stats
/// Team power is the sum of all members' (stamina + strength)
pub fn calculate_team_power_from_members(members: &[TeamMemberStats]) -> i32 {
    members
        .iter()
        .map(|member| member.stats.stamina + member.stats.strength)
        .sum()
}

/// Fetch team members with their stats
pub async fn get_team_members_with_stats(
    team_id: Uuid,
    pool: &PgPool,
) -> Result<Vec<TeamMemberStats>, sqlx::Error> {
    let results = sqlx::query!(
        r#"
        SELECT 
            tm.user_id,
            tm.status,
            COALESCE(ua.stamina, 0) as stamina,
            COALESCE(ua.strength, 0) as strength
        FROM team_members tm
        LEFT JOIN user_avatars ua ON tm.user_id = ua.user_id
        WHERE tm.team_id = $1
        "#,
        team_id
    )
    .fetch_all(pool)
    .await?;

    let members = results
        .into_iter()
        .map(|row| TeamMemberStats {
            user_id: row.user_id,
            stats: PlayerStats {
                stamina: row.stamina.unwrap_or(0),
                strength: row.strength.unwrap_or(0),
            },
            status: row.status,
        })
        .collect();

    Ok(members)
}

/// Calculate team power by fetching members and calculating
pub async fn calculate_team_power(
    team_id: Uuid,
    pool: &PgPool,
) -> Result<i32, sqlx::Error> {
    let members = get_team_members_with_stats(team_id, pool).await?;
    Ok(calculate_team_power_from_members(&members))
}

/// Calculate power for multiple teams efficiently
pub async fn calculate_multiple_team_powers(
    team_ids: &[Uuid],
    pool: &PgPool,
) -> Result<HashMap<Uuid, i32>, sqlx::Error> {
    let results = sqlx::query!(
        r#"
        SELECT 
            tm.team_id as "team_id!",
            tm.user_id as "user_id!",
            tm.status as "status!",
            COALESCE(ua.stamina, 0)::INT4 as "stamina!",
            COALESCE(ua.strength, 0)::INT4 as "strength!"
        FROM team_members tm
        LEFT JOIN user_avatars ua ON tm.user_id = ua.user_id
        WHERE tm.team_id = ANY($1)
        "#,
        &team_ids
    )
    .fetch_all(pool)
    .await?;

    // Group members by team
    let mut teams_members: HashMap<Uuid, Vec<TeamMemberStats>> = HashMap::new();
    
    // Initialize all teams with empty member lists
    for &team_id in team_ids {
        teams_members.insert(team_id, Vec::new());
    }
    
    // Group results by team_id
    for row in results {
        let member = TeamMemberStats {
            user_id: row.user_id,
            stats: PlayerStats {
                stamina: row.stamina,
                strength: row.strength,
            },
            status: row.status,
        };
        
        teams_members
            .entry(row.team_id)
            .or_insert_with(Vec::new)
            .push(member);
    }

    // Calculate power for each team
    let mut power_map = HashMap::new();
    for (team_id, members) in teams_members {
        let power = calculate_team_power_from_members(&members);
        power_map.insert(team_id, power);
    }

    Ok(power_map)
}

/// Calculate all teams' power in the league
pub async fn calculate_all_teams_power(
    pool: &PgPool,
) -> Result<HashMap<Uuid, i32>, sqlx::Error> {
    // Get all team IDs first
    let team_ids = sqlx::query!(
        "SELECT DISTINCT id FROM teams"
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| row.id)
    .collect::<Vec<_>>();

    calculate_multiple_team_powers(&team_ids, pool).await
}