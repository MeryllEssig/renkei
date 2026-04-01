# Packaging for distribution

```bash
rk package                  # create archive from current directory
rk package --bump patch     # bump patch version, then archive
rk package --bump minor     # bump minor version
rk package --bump major     # bump major version
```

Run from a directory containing `renkei.json`. Creates a `<name>-<version>.tar.gz` archive containing only:
- `renkei.json`
- `skills/`
- `hooks/`
- `agents/`
- `scripts/`

Everything else is excluded.

With `--bump`, the version in `renkei.json` is incremented before archiving (the manifest file is rewritten on disk).

Displays a summary: included files, file count, and archive size.
