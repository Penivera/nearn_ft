// In src/main.rs

use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

#[tokio::main]
async fn main() {
    env_logger::init();
    
    let target_url = "http://127.0.0.1:8080/transfer";
    let requests_per_second = 100;
    let duration_minutes = 10;
    let total_requests = requests_per_second * 60 * duration_minutes;

    let client = Client::new();
    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));

    let bar = ProgressBar::new(total_requests as u64);
    bar.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) | ETA: {eta} | Success: {msg}")
        .unwrap());

    // Create an interval to fire `requests_per_second` times every second
    let mut interval = interval(Duration::from_secs(1));

    for i in 0.. (60 * duration_minutes) {
        interval.tick().await; // Wait for the next second

        let mut tasks = Vec::new();

        for j in 0..requests_per_second {
            let client = client.clone();
            let success_count = Arc::clone(&success_count);
            let error_count = Arc::clone(&error_count);
            let bar = bar.clone();
            
            // Generate a unique receiver for each request
            let receiver_id = format!("loadtest-{}-{}.test.near", i, j);

            tasks.push(tokio::spawn(async move {
                let body = serde_json::json!({
                    "reciever_id": receiver_id,
                    "amount": "1", // Send a minimal amount
                    "memo": "benchmark"
                });

                match client.post(target_url).json(&body).send().await {
                    Ok(response) if response.status().is_success() => {
                        success_count.fetch_add(1, Ordering::SeqCst);
                    }
                    _ => {
                        error_count.fetch_add(1, Ordering::SeqCst);
                    }
                }
                bar.inc(1);
            }));
        }
        futures::future::join_all(tasks).await;
        bar.set_message(format!("{}", success_count.load(Ordering::SeqCst)));
    }

    bar.finish_with_message("Benchmark complete!".to_string());

    println!("\n--- Benchmark Results ---");
    println!("Total Requests Sent: {}", total_requests);
    println!("Successful Requests: {}", success_count.load(Ordering::SeqCst));
    println!("Failed Requests: {}", error_count.load(Ordering::SeqCst));
}