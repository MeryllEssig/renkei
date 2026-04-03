# Listing installed packages

```bash
rk list          # project-scoped packages (default)
rk list -g       # globally installed packages
```

Displays a table with: package name, version, source type (`[git]` or `[local]`), and artifact types.

If no packages are installed, shows "No packages installed." and exits with code 0.
