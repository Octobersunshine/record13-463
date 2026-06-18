mod error;
mod export;
mod lottery;

use axum::http::Method;
use axum::routing::{get, post};
use axum::Router;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use export::{draw_and_export, download_file, ExportResponse};
use lottery::{draw, DrawRequest, DrawResponse};

#[derive(OpenApi)]
#[openapi(
    paths(lottery::draw, export::draw_and_export, export::download_file),
    components(schemas(DrawRequest, DrawResponse, ExportResponse)),
    tags((name = "lottery", description = "抽奖 API"))
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lottery_api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    export::init_export_dir().expect("初始化导出目录失败");

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(Any)
        .allow_headers(Any);

    let api_router = Router::new()
        .route("/api/draw", post(draw))
        .route("/api/draw/export", post(draw_and_export))
        .route("/api/download/:filename", get(download_file))
        .layer(cors);

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(api_router);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}
