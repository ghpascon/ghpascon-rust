use std::future::Future;
use std::time::Duration;

/// Executes a closure after a specified delay, always asynchronously.
///
/// # Arguments
///
/// * `delay` - Duration to wait before executing
/// * `func` - Async closure or function to execute after the delay
///
/// # Returns
///
/// The result of the closure.
pub async fn delayed_function<F, Fut, T>(delay: Duration, func: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    tokio::time::sleep(delay).await;
    func().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_delayed_returns_value() {
        let result = delayed_function(Duration::from_millis(10), || async { 42 }).await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_delayed_with_string() {
        let result = delayed_function(Duration::from_millis(10), || async { "hello" }).await;
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_delayed_with_bool() {
        let result = delayed_function(Duration::from_millis(10), || async { true }).await;
        assert!(result);
    }
}
