use ghpascon_rust::utils::functions::delayed_function;
use std::time::Duration;

#[tokio::main]
async fn main() {
    // spawn multiple delayed tasks concurrently
    let tasks = vec![
        tokio::spawn(async {
            delayed_function(Duration::from_millis(300), || async {
                println!("Task 1 done (300ms)");
            })
            .await;
        }),
        tokio::spawn(async {
            delayed_function(Duration::from_millis(100), || async {
                println!("Task 2 done (100ms)");
            })
            .await;
        }),
        tokio::spawn(async {
            delayed_function(Duration::from_millis(200), || async {
                println!("Task 3 done (200ms)");
            })
            .await;
        }),
    ];

    println!("All tasks spawned, waiting for completion...");

    // wait for all tasks to finish
    for task in tasks {
        task.await.unwrap();
    }

    println!("All tasks completed.");
}
