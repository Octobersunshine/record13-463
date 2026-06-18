use axum::http::StatusCode;
use axum::Router;
use axum_test::TestServer;
use lottery_api::lottery::{DrawRequest, DrawResponse};
use serde_json::json;

fn make_app() -> Router {
    use axum::routing::post;
    use lottery_api::lottery::draw;
    Router::new().route("/api/draw", post(draw))
}

#[tokio::test]
async fn test_api_draw_success() {
    let server = TestServer::new(make_app()).unwrap();

    let req = DrawRequest {
        participants: vec!["alice".into(), "bob".into(), "charlie".into(), "david".into()],
        count: 2,
        seed: Some(999),
    };

    let resp = server
        .post("/api/draw")
        .json(&req)
        .await;

    resp.assert_status(StatusCode::OK);

    let body: DrawResponse = resp.json();
    assert_eq!(body.total_participants, 4);
    assert_eq!(body.unique_participants, 4);
    assert_eq!(body.winner_count, 2);
    assert_eq!(body.winners.len(), 2);

    for winner in &body.winners {
        assert!(req.participants.contains(winner));
    }
}

#[tokio::test]
async fn test_api_draw_with_duplicate_participants() {
    let server = TestServer::new(make_app()).unwrap();

    let req = json!({
        "participants": ["alice", "bob", "alice", "charlie", "bob", "david"],
        "count": 3,
        "seed": 12345
    });

    let resp = server.post("/api/draw").json(&req).await;

    resp.assert_status(StatusCode::OK);

    let body: DrawResponse = resp.json();
    assert_eq!(body.total_participants, 6);
    assert_eq!(body.unique_participants, 4);
    assert_eq!(body.winner_count, 3);
    assert_eq!(body.winners.len(), 3);

    let mut winners_sorted = body.winners.clone();
    winners_sorted.sort();
    winners_sorted.dedup();
    assert_eq!(winners_sorted.len(), 3, "中奖者列表不能有重复");
}

#[tokio::test]
async fn test_api_draw_count_exceeds_unique() {
    let server = TestServer::new(make_app()).unwrap();

    let req = json!({
        "participants": ["user1", "user2", "user1", "user2", "user1"],
        "count": 3
    });

    let resp = server.post("/api/draw").json(&req).await;
    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_draw_empty_participants() {
    let server = TestServer::new(make_app()).unwrap();

    let req = DrawRequest {
        participants: vec![],
        count: 1,
        seed: None,
    };

    let resp = server
        .post("/api/draw")
        .json(&req)
        .await;

    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_draw_zero_count() {
    let server = TestServer::new(make_app()).unwrap();

    let req = json!({
        "participants": ["user1", "user2"],
        "count": 0
    });

    let resp = server
        .post("/api/draw")
        .json(&req)
        .await;

    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_draw_count_too_large() {
    let server = TestServer::new(make_app()).unwrap();

    let req = json!({
        "participants": ["user1", "user2"],
        "count": 10
    });

    let resp = server
        .post("/api/draw")
        .json(&req)
        .await;

    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_draw_deterministic() {
    let server = TestServer::new(make_app()).unwrap();

    let participants: Vec<String> = (1..=20).map(|i| format!("player{}", i)).collect();

    let req1 = json!({
        "participants": participants,
        "count": 5,
        "seed": 42
    });

    let req2 = json!({
        "participants": (1..=20).map(|i| format!("player{}", i)).collect::<Vec<_>>(),
        "count": 5,
        "seed": 42
    });

    let resp1 = server.post("/api/draw").json(&req1).await;
    let resp2 = server.post("/api/draw").json(&req2).await;

    let body1: DrawResponse = resp1.json();
    let body2: DrawResponse = resp2.json();

    assert_eq!(body1.winners, body2.winners);
}

#[tokio::test]
async fn test_api_draw_no_duplicates() {
    let server = TestServer::new(make_app()).unwrap();

    let participants: Vec<String> = (1..=100).map(|i| format!("u{}", i)).collect();

    let req = json!({
        "participants": participants,
        "count": 50
    });

    let resp = server.post("/api/draw").json(&req).await;
    let body: DrawResponse = resp.json();

    let mut sorted = body.winners.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), body.winners.len());
}

#[tokio::test]
async fn test_api_draw_all_participants_win() {
    let server = TestServer::new(make_app()).unwrap();

    let participants = vec!["a".into(), "b".into(), "c".into()];

    let req = json!({
        "participants": participants,
        "count": 3
    });

    let resp = server.post("/api/draw").json(&req).await;
    let body: DrawResponse = resp.json();

    assert_eq!(body.winner_count, 3);
    let mut winners = body.winners;
    winners.sort();
    assert_eq!(winners, vec!["a", "b", "c"]);
}
