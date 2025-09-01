use chrono::{Duration, Utc};
use secrecy::SecretString;
use uuid::Uuid;
use riina_backend::utils::workout_approval::WorkoutApprovalToken;

#[test]
fn test_token_generation_and_validation() {
    let secret = SecretString::new("test-secret-key-for-testing".into());
    let user_id = Uuid::new_v4();
    let workout_id = "workout123".to_string();
    let start = Utc::now();
    let end = start + Duration::hours(1);

    let token_data = WorkoutApprovalToken::new(
        user_id,
        workout_id.clone(),
        start,
        end,
        5, // 5 minutes validity
    );

    // Generate token
    let token = token_data.generate_token(&secret).unwrap();
    assert!(!token.is_empty());
    assert!(token.contains('|'));

    // Validate token
    let validated = WorkoutApprovalToken::validate_token(&token, &secret, user_id).unwrap();

    assert_eq!(validated.user_id, user_id);
    assert_eq!(validated.workout_id, workout_id);
    assert_eq!(validated.workout_start.timestamp(), start.timestamp());
    assert_eq!(validated.workout_end.timestamp(), end.timestamp());
    assert!(!validated.is_expired());
}

#[test]
fn test_expired_token() {
    let secret = SecretString::new("test-secret-key-for-testing".into());
    let user_id = Uuid::new_v4();
    
    let token_data = WorkoutApprovalToken::new(
        user_id,
        "workout123".to_string(),
        Utc::now(),
        Utc::now() + Duration::hours(1),
        -1, // Already expired
    );

    assert!(token_data.is_expired());

    let token = token_data.generate_token(&secret).unwrap();
    let result = WorkoutApprovalToken::validate_token(&token, &secret, user_id);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Token has expired");
}

#[test]
fn test_wrong_user_id() {
    let secret = SecretString::new("test-secret-key-for-testing".into());
    let user_id = Uuid::new_v4();
    let wrong_user_id = Uuid::new_v4();
    
    let token_data = WorkoutApprovalToken::new(
        user_id,
        "workout123".to_string(),
        Utc::now(),
        Utc::now() + Duration::hours(1),
        5,
    );

    let token = token_data.generate_token(&secret).unwrap();
    let result = WorkoutApprovalToken::validate_token(&token, &secret, wrong_user_id);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Token user ID does not match");
}

#[test]
fn test_tampered_token() {
    let secret = SecretString::new("test-secret-key-for-testing".into());
    let user_id = Uuid::new_v4();
    
    let token_data = WorkoutApprovalToken::new(
        user_id,
        "workout123".to_string(),
        Utc::now(),
        Utc::now() + Duration::hours(1),
        5,
    );

    let token = token_data.generate_token(&secret).unwrap();
    
    // Tamper with the token by modifying the last character of the signature
    let mut token_chars: Vec<char> = token.chars().collect();
    let last_idx = token_chars.len() - 1;
    token_chars[last_idx] = if token_chars[last_idx] == 'a' { 'b' } else { 'a' };
    let tampered_token: String = token_chars.into_iter().collect();

    let result = WorkoutApprovalToken::validate_token(&tampered_token, &secret, user_id);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Invalid token signature");
}

#[test]
fn test_invalid_signature() {
    let secret = SecretString::new("test-secret-key-for-testing".into());
    let user_id = Uuid::new_v4();
    
    let token_data = WorkoutApprovalToken::new(
        user_id,
        "workout123".to_string(),
        Utc::now(),
        Utc::now() + Duration::hours(1),
        5,
    );

    let token = token_data.generate_token(&secret).unwrap();
    
    // Try to validate with a different secret
    let wrong_secret = SecretString::new("wrong-secret".into());
    let result = WorkoutApprovalToken::validate_token(&token, &wrong_secret, user_id);
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Invalid token signature");
}

#[test]
fn test_token_format_validation() {
    let secret = SecretString::new("test-secret-key-for-testing".into());
    let user_id = Uuid::new_v4();
    
    // Test various invalid token formats
    let invalid_tokens = vec![
        "invalid",
        "invalid|token",
        "invalid|token|format",
        "invalid|token|format|missing",
        "invalid|token|format|missing|parts",
    ];
    
    for invalid_token in invalid_tokens {
        let result = WorkoutApprovalToken::validate_token(invalid_token, &secret, user_id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid token format");
    }
}

#[test]
fn test_token_with_different_workout_ids() {
    let secret = SecretString::new("test-secret-key-for-testing".into());
    let user_id = Uuid::new_v4();
    let start = Utc::now();
    let end = start + Duration::hours(1);
    
    // Create two tokens with different workout IDs
    let token1_data = WorkoutApprovalToken::new(
        user_id,
        "workout1".to_string(),
        start,
        end,
        5,
    );
    
    let token2_data = WorkoutApprovalToken::new(
        user_id,
        "workout2".to_string(),
        start,
        end,
        5,
    );
    
    let token1 = token1_data.generate_token(&secret).unwrap();
    let token2 = token2_data.generate_token(&secret).unwrap();
    
    // Tokens should be different
    assert_ne!(token1, token2);
    
    // Each token should validate correctly
    let validated1 = WorkoutApprovalToken::validate_token(&token1, &secret, user_id).unwrap();
    let validated2 = WorkoutApprovalToken::validate_token(&token2, &secret, user_id).unwrap();
    
    assert_eq!(validated1.workout_id, "workout1");
    assert_eq!(validated2.workout_id, "workout2");
}