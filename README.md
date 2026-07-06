# OKF-Action

GitHub Action for validating [Open Knowledge Format (OKF)](https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf) bundles.

Written in **Rust** using **Cargo**.

---

## What it validates (OKF v0.1 §9 Conformance)

| Rule | Description |
|------|-------------|
| §9 rule 1 | Every non-reserved `.md` file must contain a parseable YAML frontmatter block. |
| §9 rule 2 | Every such frontmatter block must have a non-empty `type` field. |
| §6 + §11  | `index.md` files must not have frontmatter except at the bundle root (where `okf_version` is permitted). |
| §7        | Level-2 headings in `log.md` must use ISO 8601 `YYYY-MM-DD` date format. |

Reserved filenames (`index.md`, `log.md`) are validated according to their own rules and are not required to carry `type` frontmatter.

---

## Usage

### Validate the current repository

```yaml
- uses: JackFliegel/OKF-Action@main
  with:
    bundle_path: '.'        # path to bundle, relative to repo root (default: '.')
```

### Validate a subdirectory

```yaml
- uses: JackFliegel/OKF-Action@main
  with:
    bundle_path: 'okf/bundles/my-bundle'
```

### Validate an external repository

```yaml
- uses: JackFliegel/OKF-Action@main
  with:
    repo: 'https://github.com/GoogleCloudPlatform/knowledge-catalog'
    bundle_path: 'okf/bundles'
```

---

## Inputs

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `bundle_path` | No | `.` | Path to the OKF bundle directory, relative to the repository root (or to the cloned repo root when `repo` is set). |
| `repo` | No | `''` | External GitHub repository URL to clone and validate (e.g. `https://github.com/owner/repo`). |

## Outputs

| Output | Description |
|--------|-------------|
| `files_checked` | Number of Markdown files inspected. |
| `error_count` | Number of OKF conformance errors found. |

---

## Development

### Prerequisites

- [Rust](https://rustup.rs/) 1.82+
- [Docker](https://www.docker.com/) (for building/testing the Action container)

### Build

```bash
cargo build --release
```

### Test

```bash
cargo test
```

### Run locally

```bash
# Validate the current directory as an OKF bundle
INPUT_BUNDLE_PATH=. GITHUB_WORKSPACE=$(pwd) ./target/release/okf-action

# Validate an external repo
INPUT_REPO=https://github.com/GoogleCloudPlatform/knowledge-catalog \
INPUT_BUNDLE_PATH=okf/bundles \
  ./target/release/okf-action
```

---

## OKF Specification

See the [OKF v0.1 spec](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md) for the full format definition.
