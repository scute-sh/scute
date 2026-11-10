# CI

Gate pull requests on the files they change, and keep an eye on what drifts
between them. The examples use GitHub Actions; any CI that runs shell
commands works the same way.

## On pull requests

```yaml
on: pull_request

jobs:
  scute:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0
      # install scute, see the README
      - run: git diff -z --diff-filter=d --name-only origin/${{ github.base_ref }}...HEAD | xargs -0r scute check code-complexity
      - run: git diff -z --diff-filter=d --name-only origin/${{ github.base_ref }}...HEAD | xargs -0r scute check code-similarity
      - run: scute check commit-message "$PR_TITLE"
        env:
          PR_TITLE: ${{ github.event.pull_request.title }}
```

- `fetch-depth: 0` fetches the base branch, so `git diff` can compare against
  it. Without it, the checkout only has the PR's last commit.
- `origin/<base>...HEAD` lists what the PR changed since it branched off.
- The rest works like in a [git hook](git-hooks.md): every changed file goes
  in, deleted ones stay out, and `xargs -0r` skips a check when the PR changes
  no files. [Choosing files](../input.md) has the details.
- Checking the PR title fits teams that squash-merge, where the title becomes
  the commit message. It goes through `env` rather than straight into the
  script, so a crafted title can't run commands in your CI.

A step fails when its check fails or errors, and passes on warnings. See
[Reading results](../output.md) for the exit codes.

## On a schedule

Some checks matter over time rather than per change. Dependencies drift
whether you're shipping or not:

```yaml
on:
  schedule:
    - cron: "0 9 * * 1" # every Monday at 9am

jobs:
  scute:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      # install scute, see the README
      - run: scute check dependency-freshness
```
