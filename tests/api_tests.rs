use axum::http::StatusCode;
use axum::Router;
use axum_test::TestServer;
use lottery_api::export::ExportResponse;
use lottery_api::lottery::{DrawRequest, DrawResponse};
use serde_json::json;

fn make_app() -> Router {
    use axum::routing::{get, post};
    use lottery_api::export::{draw_and_export, download_file};
    use lottery_api::lottery::draw;

    let _ = lottery_api::export::init_export_dir();

    Router::new()
        .route("/api/draw", post(draw))
        .route("/api/draw/export", post(draw_and_export))
        .route("/api/download/:filename", get(download_file))
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

#[tokio::test]
async fn test_api_draw_and_export_success() {
    let server = TestServer::new(make_app()).unwrap();

    let req = json!({
        "participants": ["alice", "bob", "charlie", "david", "eve"],
        "count": 3,
        "seed": 12345
    });

    let resp = server.post("/api/draw/export").json(&req).await;

    resp.assert_status(StatusCode::OK);

    let body: ExportResponse = resp.json();
    assert!(body.filename.ends_with(".xlsx"));
    assert!(body.download_url.starts_with("/api/download/"));
    assert!(body.full_url.contains("localhost"));
    assert_eq!(body.draw_result.total_participants, 5);
    assert_eq!(body.draw_result.winner_count, 3);

    let export_dir = std::env::current_dir().unwrap().join("exports");
    let filepath = export_dir.join(&body.filename);
    assert!(filepath.exists(), "导出的 Excel 文件不存在");

    let metadata = std::fs::metadata(&filepath).unwrap();
    assert!(metadata.len() > 1000, "Excel 文件太小");

    let _ = std::fs::remove_file(&filepath);
}

#[tokio::test]
async fn test_api_download_file_success() {
    let server = TestServer::new(make_app()).unwrap();

    let export_req = json!({
        "participants": ["user1", "user2", "user3"],
        "count": 2,
        "seed": 999
    });

    let export_resp = server.post("/api/draw/export").json(&export_req).await;
    let export_body: ExportResponse = export_resp.json();
    let filename = export_body.filename.clone();

    let download_resp = server.get(&format!("/api/download/{}", filename)).await;
    download_resp.assert_status(StatusCode::OK);

    let content_type = download_resp.header("content-type");
    assert!(content_type.contains("vnd.openxmlformats"));

    let disposition = download_resp.header("content-disposition");
    assert!(disposition.contains("attachment"));
    assert!(disposition.contains(&filename));

    let export_dir = std::env::current_dir().unwrap().join("exports");
    let filepath = export_dir.join(&filename);
    let _ = std::fs::remove_file(&filepath);
}

#[tokio::test]
async fn test_api_download_file_not_found() {
    let server = TestServer::new(make_app()).unwrap();

    let resp = server.get("/api/download/nonexistent_file.xlsx").await;
    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_download_file_path_traversal() {
    let server = TestServer::new(make_app()).unwrap();

    let resp = server.get("/api/download/../../../etc/passwd.xlsx").await;
    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_download_file_wrong_extension() {
    let server = TestServer::new(make_app()).unwrap();

    let resp = server.get("/api/download/malicious.exe").await;
    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_draw_and_export_with_duplicates() {
    let server = TestServer::new(make_app()).unwrap();

    let req = json!({
        "participants": ["alice", "bob", "alice", "charlie", "bob", "david", "alice"],
        "count": 3,
        "seed": 42
    });

    let resp = server.post("/api/draw/export").json(&req).await;
    resp.assert_status(StatusCode::OK);

    let body: ExportResponse = resp.json();
    assert_eq!(body.draw_result.total_participants, 7);
    assert_eq!(body.draw_result.unique_participants, 4);
    assert_eq!(body.draw_result.winner_count, 3);

    let mut winners = body.draw_result.winners.clone();
    winners.sort();
    winners.dedup();
    assert_eq!(winners.len(), 3);

    let export_dir = std::env::current_dir().unwrap().join("exports");
    let filepath = export_dir.join(&body.filename);
    let _ = std::fs::remove_file(&filepath);
}
