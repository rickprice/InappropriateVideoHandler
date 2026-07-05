---
name: feedback_update_docs
description: Always update README, tests, and config files alongside any code change
metadata:
  type: feedback
---

Always update README.md, tests, and configuration files (config.yaml, test_config.yaml) any time a code change is made.

**Why:** User explicitly requested this as a standing rule.

**How to apply:** After every code change — including defaults, behavior, flags, or logic — check and update:
- README.md (feature descriptions, config reference table, "How It Works", example snippets)
- Unit and integration tests to match new defaults or behavior
- config.yaml and test_config.yaml if defaults or structure changed
