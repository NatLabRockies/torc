---
name: Fix tests not production code
description: When a test fails due to a code change, fix or delete the test rather than reverting/weakening the production code
type: feedback
---

When a test fails because production code was intentionally changed, fix or delete the test — do NOT
weaken the production code to make the test pass.

**Why:** The user got frustrated when I tried to change an `expect()` in production code to `.ok()`
just to make a test pass. The production code was correct; the test was outdated.

**How to apply:** When a test fails after intentional code changes, always ask whether to fix the
test or confirm before modifying production code. Default to updating the test.
