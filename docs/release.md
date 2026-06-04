# Release

Release automation is intentionally deferred while `nexac parse` and `nexac check` are still stabilizing.

Before restoring release workflows:

1. Decide which crates are publishable.
2. Confirm package metadata and crate descriptions.
3. Add dry-run publishing in dependency order.
4. Add binary packaging for `nexac`.
5. Require normal CI checks before publishing artifacts.
