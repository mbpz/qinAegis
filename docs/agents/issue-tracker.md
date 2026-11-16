# Issue tracker: GitHub

Updated: 2026-11-16. Product scope follows [CONTEXT.md](../../CONTEXT.md), the [2.0 plan](../plans/local-web.md), and [verified progress](../progress/local-web-2.0.md). Track contract reuse/governance, local Web delivery, extensible model routing, Playwright + Midscene.js execution and Allure separately from planned orchestration or accessibility capabilities. An issue closure does not certify the entire 2.0 release.

These are tool conventions. Creating issues, posting comments or changing labels requires authorization from the user or an explicitly invoked workflow; reading tickets does not imply permission to publish.

Issues and PRDs for this repo live as GitHub issues. Use the `gh` CLI for all operations.

## Conventions

- **Create an issue**: `gh issue create --title "..." --body "..."`. Use a heredoc for multi-line bodies.
- **Read an issue**: `gh issue view <number> --comments`, filtering comments by `jq` and also fetching labels.
- **List issues**: `gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'` with appropriate `--label` and `--state` filters.
- **Comment on an issue**: `gh issue comment <number> --body "..."`
- **Apply / remove labels**: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **Close**: `gh issue close <number> --comment "..."`

Infer the repo from `git remote -v` — `gh` does this automatically when run inside a clone.

## When a skill says "publish to the issue tracker"

Create a GitHub issue.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> --comments`.
