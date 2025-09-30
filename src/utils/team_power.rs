// src/utils/team_power.rs
use sqlx::PgPool;
use uuid::Uuid;
use std::collections::HashMap;
use crate::models::common::PlayerStats;

/// Team member with stats for power calculation
#[derive(Debug)]
pub struct TeamMemberStats {
    pub stats: PlayerStats,
}

/// Calculate team power based on member stats
/// Team power is the sum of all members' (stamina + strength)
pub fn calculate_team_power_from_members(members: &[TeamMemberStats]) -> f32 {
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
    // First, get all team members
    let members = sqlx::query!(
        r#"
        SELECT user_id, status
        FROM team_members
        WHERE team_id = $1
        "#,
        team_id
    )
    .fetch_all(pool)
    .await?;

    // Then, fetch stats for each member separately
    let mut team_members = Vec::new();
    
    for member in members {
        // Get user avatar stats if they exist
        let stats = sqlx::query!(
            r#"
            SELECT stamina, strength
            FROM user_avatars
            WHERE user_id = $1
            "#,
            member.user_id
        )
        .fetch_optional(pool)
        .await?;

        let player_stats = match stats {
            Some(row) => PlayerStats {
                stamina: row.stamina,
                strength: row.strength,
            },
            None => PlayerStats {
                stamina: 0.0,
                strength: 0.0,
            },
        };

        team_members.push(TeamMemberStats {
            stats: player_stats,
        });
    }

    Ok(team_members)
}

/// Calculate team power by fetching members and calculating
pub async fn calculate_team_power(
    team_id: Uuid,
    pool: &PgPool,
) -> Result<f32, sqlx::Error> {
    let members = get_team_members_with_stats(team_id, pool).await?;
    Ok(calculate_team_power_from_members(&members))
}

/// Calculate power for multiple teams efficiently
pub async fn calculate_multiple_team_powers(
    team_ids: &[Uuid],
    pool: &PgPool,
) -> Result<HashMap<Uuid, f32>, sqlx::Error> {
    // Get all team members for the given teams
    let members = sqlx::query!(
        r#"
        SELECT team_id, user_id, status
        FROM team_members
        WHERE team_id = ANY($1)
        "#,
        &team_ids
    )
    .fetch_all(pool)
    .await?;

    // Collect all unique user IDs
    let user_ids: Vec<Uuid> = members.iter().map(|m| m.user_id).collect();
    
    // Get all avatar stats in one query
    let avatar_stats = sqlx::query!(
        r#"
        SELECT user_id, stamina, strength
        FROM user_avatars
        WHERE user_id = ANY($1)
        "#,
        &user_ids
    )
    .fetch_all(pool)
    .await?;

    // Create a map of user_id to stats
    let mut stats_map: HashMap<Uuid, PlayerStats> = HashMap::new();
    for stat in avatar_stats {
        stats_map.insert(stat.user_id, PlayerStats {
            stamina: stat.stamina,
            strength: stat.strength,
        });
    }

    // Group members by team
    let mut teams_members: HashMap<Uuid, Vec<TeamMemberStats>> = HashMap::new();
    
    // Initialize all teams with empty member lists
    for &team_id in team_ids {
        teams_members.insert(team_id, Vec::new());
    }
    
    // Group members by team_id
    for member in members {
        let stats = stats_map.get(&member.user_id).cloned().unwrap_or(PlayerStats {
            stamina: 0.0,
            strength: 0.0,
        });

        let team_member = TeamMemberStats {
            stats,
        };
        
        teams_members
            .entry(member.team_id)
            .or_default()
            .push(team_member);
    }

    // Calculate power for each team
    let mut power_map: HashMap<Uuid, f32> = HashMap::new();
    for (team_id, members) in teams_members {
        let power = calculate_team_power_from_members(&members);
        power_map.insert(team_id, power);
    }

    Ok(power_map)
}

