use crate::error::AppError;
use axum::Json;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
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
    if req.participants.is_empty() {
        return Err(AppError::BadRequest("参与者列表不能为空".into()));
    }

    if req.count == 0 {
        return Err(AppError::BadRequest("中奖数量必须大于 0".into()));
    }

    if req.count > req.participants.len() {
        return Err(AppError::BadRequest(format!(
            "中奖数量 ({}) 不能大于参与者数量 ({})",
            req.count,
            req.participants.len()
        )));
    }

    let mut rng = match req.seed {
        Some(seed) => ChaCha20Rng::seed_from_u64(seed),
        None => ChaCha20Rng::from_entropy(),
    };

    let mut participants = req.participants.clone();
    participants.shuffle(&mut rng);

    let winners: Vec<String> = participants.into_iter().take(req.count).collect();

    Ok(Json(DrawResponse {
        total_participants: req.participants.len(),
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
        assert_eq!(resp.winner_count, 2);
        assert_eq!(resp.winners.len(), 2);
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
