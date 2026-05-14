// Run with: cargo run --example taglist_performance

use std::sync::Arc;
use std::time::Instant;

use ghpascon_rust::utils::tag_list::{TagList, make_tag};

const TOTAL_TAGS: usize = 500_000;

#[tokio::main]
async fn main() {
    let list = Arc::new(TagList::builder().build());

    let half = TOTAL_TAGS / 2;

    println!("=== TagList Async Performance Test ===");
    println!("Total inserts : {}", TOTAL_TAGS);
    println!("Duplicates    : {} (mesmo EPC, rssi/ant variando)", half);
    println!("Unique        : {} (EPCs distintos)", half);
    println!();

    let start = Instant::now();

    let mut handles = Vec::with_capacity(TOTAL_TAGS);

    // Metade: EPC duplicado — simula re-leituras da mesma tag
    for i in 0..half {
        let list = Arc::clone(&list);
        handles.push(tokio::spawn(async move {
            let tag = make_tag(
                "E28011606000020000000001",
                Some("E28011052000701234567890"),
                -(50 + (i % 20) as i64),
                (i % 4 + 1) as u64,
            );
            list.add(tag, "reader-dup")
        }));
    }

    // Metade: EPCs únicos — simula tags novas
    for i in 0..half {
        let list = Arc::clone(&list);
        handles.push(tokio::spawn(async move {
            let epc = format!("E280110000{:014X}", i);
            let tag = make_tag(&epc, None, -(40 + (i % 30) as i64), (i % 4 + 1) as u64);
            list.add(tag, "reader-unique")
        }));
    }

    let mut new_count = 0usize;
    let mut update_count = 0usize;

    for handle in handles {
        let (is_new, _) = handle.await.expect("task panicked");
        if is_new {
            new_count += 1;
        } else {
            update_count += 1;
        }
    }

    let elapsed = start.elapsed();

    println!("Results:");
    println!("  New insertions : {}", new_count);
    println!("  Updates        : {}", update_count);
    println!("  Final list len : {} unique keys", list.len());
    println!();
    println!("Elapsed time     : {:?}", elapsed);
    println!(
        "Throughput       : {:.0} ops/s",
        TOTAL_TAGS as f64 / elapsed.as_secs_f64()
    );
}
