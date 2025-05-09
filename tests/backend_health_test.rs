use reqwest::Client;

mod common;
use common::utils::spawn_app;

#[tokio::test]
async fn backend_health_working() {
    let test_app = spawn_app().await;
    let client = Client::new();

    let response = client
        .get(&format!("{}/backend_health", &test_app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());

    let body = response.text().await.expect("Cannot read response body.");
    let json_response: serde_json::Value = serde_json::from_str(&body).expect("Cannot turn into a json.");

    assert_eq!(json_response, serde_json::json!({
        "status": "UP"
    }));
}