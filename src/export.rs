use crate::error::AppError;
use crate::lottery::{draw, DrawRequest, DrawResponse};
use axum::body::Body;
use axum::extract::Path;
use axum::http::{header, HeaderMap, StatusCode};
use axum::Json;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::fs;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use xlsxwriter::prelude::*;

#[derive(Debug, Serialize, ToSchema)]
pub struct ExportResponse {
    #[schema(example = "lottery_20240101_120000_a1b2c3.xlsx")]
    pub filename: String,

    #[schema(example = "/api/download/lottery_20240101_120000_a1b2c3.xlsx")]
    pub download_url: String,

    #[schema(example = "http://localhost:3000/api/download/lottery_20240101_120000_a1b2c3.xlsx")]
    pub full_url: String,

    pub draw_result: DrawResponse,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DownloadPath {
    pub filename: String,
}

static EXPORT_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init_export_dir() -> Result<(), AppError> {
    let dir = std::env::current_dir()
        .map_err(|e| AppError::InternalServerError(format!("获取当前目录失败: {}", e)))?
        .join("exports");

    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::InternalServerError(format!("创建导出目录失败: {}", e)))?;
    }

    EXPORT_DIR
        .set(dir)
        .map_err(|_| AppError::InternalServerError("导出目录初始化失败".into()))?;

    Ok(())
}

fn get_export_dir() -> &'static PathBuf {
    EXPORT_DIR.get().expect("导出目录未初始化")
}

fn generate_filename() -> String {
    let now = Local::now();
    let timestamp = now.format("%Y%m%d_%H%M%S");
    let uuid = Uuid::new_v4().simple().to_string();
    format!("lottery_{}_{}.xlsx", timestamp, &uuid[..6])
}

fn create_excel(
    filepath: &PathBuf,
    draw_response: &DrawResponse,
    all_participants: &[String],
) -> Result<(), AppError> {
    let workbook = Workbook::new(filepath.to_str().unwrap())
        .map_err(|e| AppError::InternalServerError(format!("创建 Excel 工作簿失败: {}", e)))?;

    let mut bold_format = Format::new();
    bold_format.set_bold();
    bold_format.set_font_size(12);

    let mut header_format = Format::new();
    header_format.set_bold();
    header_format.set_bg_color(FormatColor::Custom(0x4472C4));
    header_format.set_font_color(FormatColor::White);
    header_format.set_align(FormatAlign::Center);
    header_format.set_border(FormatBorder::Thin);

    let mut cell_format = Format::new();
    cell_format.set_border(FormatBorder::Thin);
    cell_format.set_align(FormatAlign::Left);

    let mut title_format = Format::new();
    title_format.set_bold();
    title_format.set_font_size(16);
    title_format.set_align(FormatAlign::Center);

    let mut winners_sheet = workbook
        .add_worksheet(Some("中奖名单"))
        .map_err(|e| AppError::InternalServerError(format!("创建工作表失败: {}", e)))?;

    winners_sheet
        .set_column(0, 0, 8.0, None)
        .map_err(|e| AppError::InternalServerError(format!("设置列宽失败: {}", e)))?;
    winners_sheet
        .set_column(1, 1, 25.0, None)
        .map_err(|e| AppError::InternalServerError(format!("设置列宽失败: {}", e)))?;
    winners_sheet
        .set_column(2, 2, 20.0, None)
        .map_err(|e| AppError::InternalServerError(format!("设置列宽失败: {}", e)))?;

    winners_sheet
        .merge_range(0, 0, 0, 2, "🎉 抽奖活动中奖名单", &title_format)
        .map_err(|e| AppError::InternalServerError(format!("写入标题失败: {}", e)))?;

    winners_sheet
        .write_string(2, 0, "序号", &header_format)
        .map_err(|e| AppError::InternalServerError(format!("写入表头失败: {}", e)))?;
    winners_sheet
        .write_string(2, 1, "用户名", &header_format)
        .map_err(|e| AppError::InternalServerError(format!("写入表头失败: {}", e)))?;
    winners_sheet
        .write_string(2, 2, "中奖等级", &header_format)
        .map_err(|e| AppError::InternalServerError(format!("写入表头失败: {}", e)))?;

    for (i, winner) in draw_response.winners.iter().enumerate() {
        let row = (i + 3) as u32;
        let rank = match i {
            0 => "一等奖",
            1 => "二等奖",
            2 => "三等奖",
            _ => "幸运奖",
        };

        winners_sheet
            .write_number(row, 0, (i + 1) as f64, &cell_format)
            .map_err(|e| AppError::InternalServerError(format!("写入序号失败: {}", e)))?;
        winners_sheet
            .write_string(row, 1, winner, &cell_format)
            .map_err(|e| AppError::InternalServerError(format!("写入用户名失败: {}", e)))?;
        winners_sheet
            .write_string(row, 2, rank, &cell_format)
            .map_err(|e| AppError::InternalServerError(format!("写入中奖等级失败: {}", e)))?;
    }

    let info_row = (draw_response.winner_count + 4) as u32;
    winners_sheet
        .write_string(info_row, 0, "抽奖时间", &bold_format)
        .map_err(|e| AppError::InternalServerError(format!("写入信息失败: {}", e)))?;
    winners_sheet
        .write_string(
            info_row,
            1,
            &Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            None,
        )
        .map_err(|e| AppError::InternalServerError(format!("写入信息失败: {}", e)))?;

    winners_sheet
        .write_string(info_row + 1, 0, "参与人数", &bold_format)
        .map_err(|e| AppError::InternalServerError(format!("写入信息失败: {}", e)))?;
    winners_sheet
        .write_number(
            info_row + 1,
            1,
            draw_response.total_participants as f64,
            None,
        )
        .map_err(|e| AppError::InternalServerError(format!("写入信息失败: {}", e)))?;

    winners_sheet
        .write_string(info_row + 2, 0, "去重人数", &bold_format)
        .map_err(|e| AppError::InternalServerError(format!("写入信息失败: {}", e)))?;
    winners_sheet
        .write_number(
            info_row + 2,
            1,
            draw_response.unique_participants as f64,
            None,
        )
        .map_err(|e| AppError::InternalServerError(format!("写入信息失败: {}", e)))?;

    winners_sheet
        .write_string(info_row + 3, 0, "中奖人数", &bold_format)
        .map_err(|e| AppError::InternalServerError(format!("写入信息失败: {}", e)))?;
    winners_sheet
        .write_number(
            info_row + 3,
            1,
            draw_response.winner_count as f64,
            None,
        )
        .map_err(|e| AppError::InternalServerError(format!("写入信息失败: {}", e)))?;

    let mut participants_sheet = workbook
        .add_worksheet(Some("参与名单"))
        .map_err(|e| AppError::InternalServerError(format!("创建工作表失败: {}", e)))?;

    participants_sheet
        .set_column(0, 0, 8.0, None)
        .map_err(|e| AppError::InternalServerError(format!("设置列宽失败: {}", e)))?;
    participants_sheet
        .set_column(1, 1, 30.0, None)
        .map_err(|e| AppError::InternalServerError(format!("设置列宽失败: {}", e)))?;

    participants_sheet
        .write_string(0, 0, "序号", &header_format)
        .map_err(|e| AppError::InternalServerError(format!("写入表头失败: {}", e)))?;
    participants_sheet
        .write_string(0, 1, "用户名", &header_format)
        .map_err(|e| AppError::InternalServerError(format!("写入表头失败: {}", e)))?;

    for (i, participant) in all_participants.iter().enumerate() {
        let row = (i + 1) as u32;
        participants_sheet
            .write_number(row, 0, (i + 1) as f64, &cell_format)
            .map_err(|e| AppError::InternalServerError(format!("写入序号失败: {}", e)))?;
        participants_sheet
            .write_string(row, 1, participant, &cell_format)
            .map_err(|e| AppError::InternalServerError(format!("写入用户名失败: {}", e)))?;
    }

    workbook
        .close()
        .map_err(|e| AppError::InternalServerError(format!("保存 Excel 文件失败: {}", e)))?;

    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/draw/export",
    request_body = DrawRequest,
    responses(
        (status = 200, description = "抽奖并导出成功", body = ExportResponse),
        (status = 400, description = "请求参数错误"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "lottery"
)]
pub async fn draw_and_export(
    Json(req): Json<DrawRequest>,
) -> Result<Json<ExportResponse>, AppError> {
    let participants_clone = req.participants.clone();

    let draw_resp = draw(Json(req)).await?.0;

    let filename = generate_filename();
    let filepath = get_export_dir().join(&filename);

    create_excel(&filepath, &draw_resp, &participants_clone)?;

    let download_url = format!("/api/download/{}", filename);
    let full_url = format!("http://localhost:3000{}", download_url);

    Ok(Json(ExportResponse {
        filename,
        download_url,
        full_url,
        draw_result: draw_resp,
    }))
}

#[utoipa::path(
    get,
    path = "/api/download/{filename}",
    params(
        ("filename" = String, Path, description = "要下载的文件名")
    ),
    responses(
        (status = 200, description = "文件下载成功", content_type = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        (status = 404, description = "文件不存在"),
        (status = 400, description = "文件名非法")
    ),
    tag = "lottery"
)]
pub async fn download_file(
    Path(filename): Path<String>,
) -> Result<(HeaderMap, Body), AppError> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(AppError::BadRequest("非法的文件名".into()));
    }

    if !filename.ends_with(".xlsx") {
        return Err(AppError::BadRequest("仅支持下载 xlsx 文件".into()));
    }

    let filepath = get_export_dir().join(&filename);

    if !filepath.exists() {
        return Err(AppError::BadRequest("文件不存在".into()));
    }

    let file_content = fs::read(&filepath)
        .await
        .map_err(|e| AppError::InternalServerError(format!("读取文件失败: {}", e)))?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            .parse()
            .unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        file_content.len().to_string().parse().unwrap(),
    );

    Ok((headers, Body::from(file_content)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_filename_format() {
        let filename = generate_filename();
        assert!(filename.starts_with("lottery_"));
        assert!(filename.ends_with(".xlsx"));
        assert!(filename.len() > 20);
    }

    #[tokio::test]
    async fn test_create_excel_success() {
        let _ = init_export_dir();

        let draw_resp = DrawResponse {
            winners: vec!["user1".into(), "user2".into(), "user3".into()],
            total_participants: 10,
            unique_participants: 8,
            winner_count: 3,
        };

        let participants = vec![
            "user1".into(),
            "user2".into(),
            "user3".into(),
            "user4".into(),
            "user5".into(),
        ];

        let filename = generate_filename();
        let filepath = get_export_dir().join(&filename);

        let result = create_excel(&filepath, &draw_resp, &participants);
        assert!(result.is_ok(), "创建 Excel 失败: {:?}", result.err());

        assert!(filepath.exists(), "Excel 文件未生成");

        let metadata = std::fs::metadata(&filepath).unwrap();
        assert!(metadata.len() > 1000, "Excel 文件太小，可能为空");

        let _ = std::fs::remove_file(&filepath);
    }

    #[tokio::test]
    async fn test_draw_and_export_success() {
        let _ = init_export_dir();

        let req = DrawRequest {
            participants: vec![
                "alice".into(),
                "bob".into(),
                "charlie".into(),
                "david".into(),
            ],
            count: 2,
            seed: Some(42),
        };

        let result = draw_and_export(Json(req)).await;
        assert!(result.is_ok(), "抽奖导出失败: {:?}", result.err());

        let resp = result.unwrap().0;
        assert!(resp.filename.ends_with(".xlsx"));
        assert!(resp.download_url.starts_with("/api/download/"));
        assert!(resp.full_url.contains("localhost"));
        assert_eq!(resp.draw_result.winner_count, 2);
        assert_eq!(resp.draw_result.total_participants, 4);

        let filepath = get_export_dir().join(&resp.filename);
        assert!(filepath.exists());

        let _ = std::fs::remove_file(&filepath);
    }

    #[tokio::test]
    async fn test_download_file_invalid_filename() {
        let _ = init_export_dir();

        let result = download_file(Path("../secret.txt".into())).await;
        assert!(result.is_err());

        let result = download_file(Path("file.exe".into())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_download_file_not_found() {
        let _ = init_export_dir();

        let result = download_file(Path("nonexistent.xlsx".into())).await;
        assert!(result.is_err());
    }
}
