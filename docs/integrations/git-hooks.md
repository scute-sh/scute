# Git hooks

Run checks on every commit, whether you're committing or your coding agent is.

## Check the staged files

```sh
#!/bin/sh
# .git/hooks/pre-commit
set -e
git diff --cached -z --diff-filter=d --name-only | xargs -0r scute check code-complexity
git diff --cached -z --diff-filter=d --name-only | xargs -0r scute check code-similarity
```

Hand over every staged file, whatever its language: the checks skip the files
they don't support.

- `--diff-filter=d` leaves deleted files out, since a check reports a missing
  file as an error.
- `xargs -r` skips the check when nothing's left to hand over (a commit that
  only deletes files, say). With no files, a check covers the whole project.
- `-z` and `-0` keep names with spaces, quotes, or accents intact.

[Choosing files](../input.md) has the details.

## Check the commit message

```sh
#!/bin/sh
# .git/hooks/commit-msg
set -e
scute check commit-message "$(cat "$1")"
```

Git passes the hook the path of the file holding the message.

## Install the hooks

Save each script under `.git/hooks/` with the name shown, and make it
executable:

```sh
chmod +x .git/hooks/pre-commit .git/hooks/commit-msg
```

`.git/hooks/` isn't versioned. To share hooks with your team, commit them to a
directory like `.githooks/` and point git at it once per clone:

```sh
git config core.hooksPath .githooks
```

A check that fails or errors stops the commit. See [Reading results](../output.md)
for what each exit code means.
