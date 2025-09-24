use chrono::{DateTime, Utc, Duration};
use sqlx::{PgPool, Row};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to database
    let database_url = "postgresql://postgres:password@localhost:5432/waiver_exchange";
    let pool = PgPool::connect(database_url).await?;

    // Clear existing data for account 7
    sqlx::query("DELETE FROM daily_equity_snapshots WHERE account_id = 7")
        .execute(&pool)
        .await?;

    println!("Cleared existing data for account 7");

    // Generate intraday equity data for today (every 5 minutes from 9:30 AM to 4:00 PM)
    let today = Utc::now().date_naive();
    let opening_time = today.and_hms_opt(9, 30, 0).unwrap();
    let opening_equity = 332000; // $3,320 in cents

    let mut current_time = opening_time;
    let mut current_equity = opening_equity;
    let mut day_change = 0;

    // Generate 78 data points (every 5 minutes for 6.5 hours)
    for i in 0..78 {
        let equity_variation = if i == 0 {
            0 // Opening
        } else {
            // Realistic 5-minute variations (-$50 to +$50)
            (rand::random::<f64>() - 0.5) * 10000.0 as i64
        };

        current_equity = (current_equity as f64 + equity_variation as f64) as i64;
        day_change = current_equity - opening_equity;
        let day_change_percent = if opening_equity > 0 {
            (day_change as f64 / opening_equity as f64) * 100.0
        } else {
            0.0
        };

        // Insert the data point
        sqlx::query(
            "INSERT INTO daily_equity_snapshots (account_id, date, total_equity, cash_balance, position_value, day_change, day_change_percent) 
             VALUES ($1, $2, $3, $4, $5, $6, $7)"
        )
        .bind(7) // account_id
        .bind(current_time)
        .bind(current_equity)
        .bind(200000) // cash_balance (fixed at $2,000)
        .bind(current_equity - 200000) // position_value
        .bind(day_change)
        .bind(day_change_percent)
        .execute(&pool)
        .await?;

        // Move to next 5-minute interval
        current_time = current_time + Duration::minutes(5);
    }

    println!("Inserted 78 intraday data points for account 7");

    // Verify the data
    let rows = sqlx::query("SELECT date, total_equity, day_change, day_change_percent FROM daily_equity_snapshots WHERE account_id = 7 ORDER BY date DESC LIMIT 5")
        .fetch_all(&pool)
        .await?;

    println!("\nLast 5 data points:");
    for row in rows {
        let date: DateTime<Utc> = row.get("date");
        let total_equity: i64 = row.get("total_equity");
        let day_change: i64 = row.get("day_change");
        let day_change_percent: f64 = row.get("day_change_percent");
        
        println!("{}: ${:.2} (${:.2}, {:.2}%)", 
                 date.format("%H:%M"), 
                 total_equity as f64 / 100.0,
                 day_change as f64 / 100.0,
                 day_change_percent);
    }

    Ok(())
}
