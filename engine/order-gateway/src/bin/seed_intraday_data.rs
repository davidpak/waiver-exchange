use chrono::{Utc, Duration, NaiveDateTime};
use sqlx::{PgPool, Row};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to database
    let database_url = "postgresql://postgres:password@localhost:5432/waiver_exchange";
    let pool = PgPool::connect(database_url).await?;

    // Clear existing data for account 7 for today
    let today = Utc::now().date_naive();
    sqlx::query("DELETE FROM daily_equity_snapshots WHERE account_id = 7 AND DATE(date) = $1")
        .bind(today)
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

    // Predefined realistic equity progression for the day
    let equity_progression = vec![
        0, 1200, 2800, 1500, 3100, 4800, 6200, 7500, 5800, 8100, 10300, 9000, 11500, 13200, 12000, 14800, 16500, 15200, 17800, 19500, 18200, 20800, 22500, 21100, 23600, 25200, 23800, 26300, 27800, 26400, 28900, 30500, 29100, 31600, 33200, 31800, 34300, 35900, 34500, 37000, 38600, 37200, 39700, 41300, 39900, 42400, 44000, 42600, 45100, 46700, 45300, 47800, 49400, 48000, 50500, 52100, 50700, 53200, 54800, 53400, 55900, 57500, 56100, 58600, 60200, 58800, 61300, 62900, 61500, 64000, 65600, 64200, 66700, 68300, 66900, 69400, 71000, 69600, 72100
    ];

    // Generate 78 data points (every 5 minutes for 6.5 hours)
    for i in 0..78 {
        let equity_change = equity_progression.get(i).copied().unwrap_or(0);
        current_equity = opening_equity + equity_change;
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
        let date: NaiveDateTime = row.get("date");
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
