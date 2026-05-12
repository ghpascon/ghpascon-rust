// Run with: cargo run --example example_tag_list

use ghpascon_rust::utils::tag_list::{Tag, TagList, make_tag};

use serde_json::json;

fn log_section(title: &str) {
    println!("\n=== {} ===", title);
}

fn log_tag(label: &str, tag: &Tag) {
    println!("{}", label);
    println!(
        "{}",
        serde_json::to_string_pretty(&*tag.lock().unwrap()).expect("json")
    );
}

fn log_list_state(list: &TagList, label: &str) {
    log_section(label);
    println!("len(): {}", list.len());
    println!(
        "get_epcs(): {}",
        serde_json::to_string_pretty(&list.get_epcs()).expect("json")
    );
    println!(
        "get_n_epcs(2): {}",
        serde_json::to_string_pretty(&list.get_n_epcs(2)).expect("json")
    );
    println!(
        "get_all(): {}",
        serde_json::to_string_pretty(&list.get_all()).expect("json")
    );
}

fn main() {
    let list = TagList::builder().prefix(vec!["E28011", "E28069"]).build();

    log_section("1. First suitcase scan");
    let (is_new, tag) = list.add(
        make_tag(
            "E28011606000020000000001",
            Some("E28011052000701234567890"),
            -58,
            1,
        ),
        "dock-reader-01",
    );
    println!("is_new: {}", is_new);
    let tag = tag.expect("first add must be valid");
    log_tag("stored tag", &tag);
    log_list_state(&list, "1.1 Collection snapshot after first add");

    // Python-like in-place enrichment via the returned reference
    {
        let mut record = tag.lock().unwrap();
        record.insert("description".to_string(), json!("Blue suitcase"));
        record.insert("status".to_string(), json!("checked-in"));
        record.insert("owner".to_string(), json!("Operations Team"));
        println!("in-place enrichment applied on stored record");
    }
    log_list_state(
        &list,
        "1.2 Collection snapshot after enriching the stored record in place",
    );

    log_section("2. Same suitcase seen again");
    let (is_new, tag) = list.add(
        make_tag(
            "E28011606000020000000001",
            Some("E28011052000701234567890"),
            -54,
            2,
        ),
        "dock-reader-02",
    );
    println!("is_new: {}", is_new);
    let tag = tag.expect("second add must be valid");
    log_tag("updated tag", &tag);
    log_list_state(&list, "2.1 Collection snapshot after second add");

    log_section("3. Same EPC, different TID");
    let (is_new, tag) = list.add(
        make_tag(
            "E28011606000020000000001",
            Some("E28011052000701234567891"),
            -51,
            1,
        ),
        "dock-reader-01",
    );
    println!("is_new: {}", is_new);
    let tag = tag.expect("third add must be valid");
    log_tag("new tag with same EPC", &tag);
    log_list_state(&list, "3.1 Collection snapshot after third add");

    log_section("4. Tag without TID");
    let (is_new, tag) = list.add(
        make_tag("E28069940000000000000002", None, -60, 3),
        "dock-reader-03",
    );
    println!("is_new: {}", is_new);
    let tag = tag.expect("null tid add must be valid");
    log_tag("tag with _epc key", &tag);
    log_list_state(&list, "4.1 Collection snapshot after fourth add");

    log_section("5. Lookups");
    println!(
        "keys linked to EPC E28011606000020000000001: {:?}",
        list.get_tids_from_epc("E28011606000020000000001")
    );

    if let Some(by_key) = list.get_by_key("E28011052000701234567890") {
        log_tag("lookup by internal key", &by_key);
    }

    if let Some(by_tid) = list.get_by_tid("E28011052000701234567890") {
        log_tag("lookup by tid", &by_tid);
    }

    if let Some(by_epc) = list.get_by_epc("E28011606000020000000001") {
        log_tag("lookup by epc", &by_epc);
    }

    log_section("6. Collection summary");
    println!("len: {}", list.len());
    println!("is_empty: {}", list.is_empty());
    println!(
        "contains key: {}",
        list.contains_key("E28011052000701234567890")
    );
    println!(
        "get_n(2) raw: {}",
        serde_json::to_string_pretty(&list.get_n(2)).expect("json")
    );
    println!(
        "get_epcs() raw: {}",
        serde_json::to_string_pretty(&list.get_epcs()).expect("json")
    );
    println!(
        "get_n_epcs(2) raw: {}",
        serde_json::to_string_pretty(&list.get_n_epcs(2)).expect("json")
    );
    println!(
        "get_n_sorted(2) raw: {}",
        serde_json::to_string_pretty(&list.get_n_sorted(2)).expect("json")
    );
    println!(
        "get_all_sorted() raw: {}",
        serde_json::to_string_pretty(&list.get_all_sorted()).expect("json")
    );

    log_section("7. Removal demo");
    let removed = list.remove_by_key("E28011052000701234567891");
    println!("removed by key: {}", removed.is_some());
    println!(
        "remaining EPC matches: {:?}",
        list.get_tids_from_epc("E28011606000020000000001")
    );
    log_list_state(&list, "7.1 Collection snapshot after removal");

    log_section("8. get_all snapshot after operations");
    println!("total records now: {}", list.len());
    println!(
        "get_all(): {}",
        serde_json::to_string_pretty(&list.get_all()).expect("json")
    );
}
