/// Only the engine's explicit post-commit status is safe to acknowledge. Ordinary
/// buffer errors and failed commits must still fail the append and roll it back.
pub(super) fn is_durable(error: &str) -> bool {
    error.contains("Transaction committed successfully, but the post-commit checkpoint failed.")
        && error.contains("The committed data is durable and will be recovered on restart:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_post_commit_failure_is_not_a_failed_append() {
        assert!(is_durable(
            "Query execution failed: Transaction committed successfully, but the post-commit checkpoint failed. The committed data is durable and will be recovered on restart: Buffer manager exception: Unable to allocate memory!"
        ));
        for error in [
            "Buffer manager exception: Unable to allocate memory!",
            "COMMIT failed",
            "Transaction committed successfully",
        ] {
            assert!(!is_durable(error));
        }
    }
}
