use crate::error::AppError;
use axum::Json;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, ToSchema)]
pub struct DrawRequest {
    #[schema(example = json!(["user1", "user2", "user3", "user4", "user5"]))]
    pub participants: Vec<String>,

    #[schema(example = 2, minimum = 1)]
    pub count: usize,

    #[schema(example = 42)]
    pub seed: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DrawResponse {
    #[schema(example = json!(["user3", "user1"]))]
    pub winners: Vec<String>,

    #[schema(example = 5)]
    pub total_participants: usize,

    #[schema(example = 5)]
    pub unique_participants: usize,

    #[schema(example = 2)]
    pub winner_count: usize,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DrawQuery {
    pub seed: Option<u64>,
}

#[utoipa::path(
    post,
    path = "/api/draw",
    request_body = DrawRequest,
    responses(
        (status = 200, description = "抽奖成功", body = DrawResponse),
        (status = 400, description = "请求参数错误")
    ),
    tag = "lottery"
)]
pub async fn draw(
    Json(req): Json<DrawRequest>,
) -> Result<Json<DrawResponse>, AppError> {
    let total_participants = req.participants.len();

    if total_participants == 0 {
        return Err(AppError::BadRequest("参与者列表不能为空".into()));
    }

    if req.count == 0 {
        return Err(AppError::BadRequest("中奖数量必须大于 0".into()));
    }

    let unique_participants: Vec<String> = req
        .participants
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let unique_count = unique_participants.len();

    if req.count > unique_count {
        return Err(AppError::BadRequest(format!(
            "中奖数量 ({}) 不能大于去重后参与者数量 ({})",
            req.count, unique_count
        )));
    }

    let mut rng = match req.seed {
        Some(seed) => ChaCha20Rng::seed_from_u64(seed),
        None => ChaCha20Rng::from_entropy(),
    };

    let mut participants = unique_participants;
    participants.shuffle(&mut rng);

    let winners: Vec<String> = participants.into_iter().take(req.count).collect();

    Ok(Json(DrawResponse {
        total_participants,
        unique_participants: unique_count,
        winner_count: winners.len(),
        winners,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_draw_success() {
        let req = DrawRequest {
            participants: vec!["a".into(), "b".into(), "c".into()],
            count: 2,
            seed: Some(42),
        };

        let result = draw(Json(req)).await;
        assert!(result.is_ok());

        let resp = result.unwrap().0;
        assert_eq!(resp.total_participants, 3);
        assert_eq!(resp.unique_participants, 3);
        assert_eq!(resp.winner_count, 2);
        assert_eq!(resp.winners.len(), 2);
    }

    #[tokio::test]
    async fn test_draw_duplicate_participants_deduped() {
        let req = DrawRequest {
            participants: vec![
                "alice".into(),
                "bob".into(),
                "alice".into(),
                "charlie".into(),
                "bob".into(),
            ],
            count: 2,
            seed: Some(42),
        };

        let result = draw(Json(req)).await;
        assert!(result.is_ok());

        let resp = result.unwrap().0;
        assert_eq!(resp.total_participants, 5);
        assert_eq!(resp.unique_participants, 3);
        assert_eq!(resp.winner_count, 2);
        assert_eq!(resp.winners.len(), 2);

        let mut winners = resp.winners.clone();
        winners.sort();
        winners.dedup();
        assert_eq!(winners.len(), 2, "中奖者不能有重复");
    }

    #[tokio::test]
    async fn test_draw_all_duplicates_only_one_unique() {
        let req = DrawRequest {
            participants: vec!["same".into(), "same".into(), "same".into()],
            count: 1,
            seed: None,
        };

        let result = draw(Json(req)).await;
        assert!(result.is_ok());

        let resp = result.unwrap().0;
        assert_eq!(resp.total_participants, 3);
        assert_eq!(resp.unique_participants, 1);
        assert_eq!(resp.winner_count, 1);
        assert_eq!(resp.winners, vec!["same"]);
    }

    #[tokio::test]
    async fn test_draw_count_exceeds_unique_participants() {
        let req = DrawRequest {
            participants: vec!["a".into(), "b".into(), "a".into(), "b".into()],
            count: 3,
            seed: None,
        };

        let result = draw(Json(req)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_draw_winners_all_unique() {
        let participants: Vec<String> = (1..=100).map(|i| format!("user{}", i)).collect();
        let mut all_participants = participants.clone();
        all_participants.extend(participants.iter().take(20).cloned());

        let req = DrawRequest {
            participants: all_participants,
            count: 50,
            seed: Some(999),
        };

        let resp = draw(Json(req)).await.unwrap().0;

        let mut winners_sorted = resp.winners.clone();
        winners_sorted.sort();
        winners_sorted.dedup();
        assert_eq!(winners_sorted.len(), resp.winners.len(), "中奖列表中不能有重复用户");
        assert_eq!(resp.unique_participants, 100);
    }

    #[tokio::test]
    async fn test_draw_empty_participants() {
        let req = DrawRequest {
            participants: vec![],
            count: 1,
            seed: None,
        };

        let result = draw(Json(req)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_draw_zero_count() {
        let req = DrawRequest {
            participants: vec!["a".into()],
            count: 0,
            seed: None,
        };

        let result = draw(Json(req)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_draw_count_exceeds_participants() {
        let req = DrawRequest {
            participants: vec!["a".into(), "b".into()],
            count: 5,
            seed: None,
        };

        let result = draw(Json(req)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_draw_deterministic_with_seed() {
        let participants: Vec<String> = (1..=10).map(|i| format!("user{}", i)).collect();

        let req1 = DrawRequest {
            participants: participants.clone(),
            count: 3,
            seed: Some(123),
        };

        let req2 = DrawRequest {
            participants,
            count: 3,
            seed: Some(123),
        };

        let resp1 = draw(Json(req1)).await.unwrap().0;
        let resp2 = draw(Json(req2)).await.unwrap().0;

        assert_eq!(resp1.winners, resp2.winners);
    }

    #[test]
    fn test_randomness_distribution() {
        use std::collections::HashMap;

        let participants: Vec<String> = (1..=10).map(|i| format!("user{}", i)).collect();
        let mut win_counts: HashMap<String, usize> = HashMap::new();

        let trials = 10000;
        for _ in 0..trials {
            let mut rng = ChaCha20Rng::from_entropy();
            let mut p = participants.clone();
            p.shuffle(&mut rng);
            for winner in p.into_iter().take(3) {
                *win_counts.entry(winner).or_insert(0) += 1;
            }
        }

        let expected = (trials * 3) as f64 / 10.0;
        for (_, count) in win_counts {
            let diff = (count as f64 - expected).abs() / expected;
            assert!(diff < 0.1, "分布偏差过大: expected {}, got {}", expected, count);
        }
    }
}
