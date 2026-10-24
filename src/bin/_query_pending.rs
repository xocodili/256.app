use sqlx::mysql::MySqlPool;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let pool = MySqlPool::connect(&std::env::var("DATABASE_URL")?).await?;
    let row: Option<(String, Option<String>, Option<String>, Option<u64>)> = sqlx::query_as(
        "SELECT status, error_code, generated_artwork_path, user_face_id FROM face_capture_pending WHERE id = '2cec0696-db5b-43fc-b3be-93e76f7900e1'"
    ).fetch_optional(&pool).await?;
    println!("complete_row={row:?}");
    Ok(())
}
