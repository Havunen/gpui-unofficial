# gpui-unofficial Design

Automated publishing of Zed's gpui framework as unofficial crates.io packages.

## Problem

Zed published gpui to crates.io once (2024) and never updated it. The framework continues evolving in the zed repo but isn't available as a versioned crate. Projects like gpuikit must use git dependencies, blocking crates.io publishing.

## Solution

An automated pipeline that:
1. Watches for new zed release tags
2. Transforms gpui crates for standalone publishing
3. Publishes to crates.io as `gpui-unofficial`, `gpui-macros-unofficial`, etc.
4. Uses an agent to fix build failures from upstream API changes

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Scope | Full ecosystem (~15-18 crates) | Maximum compatibility |
| Release trigger | Track zed tags | Clear trigger, stable releases |
| Circular dep (gpui ↔ gpui_macros) | Remove inspector feature | Simplifies publish order |
| Repo structure | Single repo, workspace | Mirrors upstream, easier to sync |
| Automation level | Fully automated + agent fixes | No human approval needed |
| Naming | `*-unofficial` suffix | Clear provenance, no ambiguity |
| Transform tooling | Rust xtask | Type-safe, same ecosystem |
| License | Apache-2.0 (inherited) | All gpui crates are Apache-2.0 |

## Repository Structure

```
gpui-unofficial/
├── .github/
│   ├── workflows/
│   │   ├── sync.yml          # Cron: check for new zed tags
│   │   ├── transform.yml     # Run transform, build, publish
│   │   └── agent-fix.yml     # gh-aw workflow for fixing failures
│   └── agents/
│       └── fix-build.md      # Agent workflow definition
├── xtask/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # CLI: cargo xtask transform|publish
│       ├── transform.rs      # Clone zed, rewrite crates
│       └── publish.rs        # Publish in dependency order
├── crates/                   # Transformed crates (gitignored, populated by xtask)
│   ├── gpui-unofficial/
│   ├── gpui-macros-unofficial/
│   └── ...
├── Cargo.toml                # Workspace root
├── .gitignore
└── README.md
```

## Crate List and Publish Order

Topologically sorted by dependencies:

### Tier 1 - Leaf crates
- `gpui-util-unofficial`
- `collections-unofficial`
- `refineable-unofficial`
- `util-macros-unofficial`

### Tier 2 - Core infrastructure
- `scheduler-unofficial`
- `sum-tree-unofficial`
- `http-client-unofficial`
- `media-unofficial`

### Tier 3 - Main crates
- `gpui-macros-unofficial` (inspector feature removed)
- `gpui-unofficial`

### Tier 4 - Platform backends
- `gpui-wgpu-unofficial`
- `gpui-macos-unofficial`
- `gpui-linux-unofficial`
- `gpui-windows-unofficial`
- `gpui-web-unofficial`

### Tier 5 - Facade
- `gpui-platform-unofficial`

## Transform Process

The `cargo xtask transform --zed-tag <tag>` command:

1. **Clone zed** at the specified tag to a temp directory
2. **Extract crates** - Copy gpui-related crates to `crates/`
3. **Rewrite Cargo.toml files:**
   - Resolve `workspace = true` to actual values
   - Rename packages: `gpui` → `gpui-unofficial`
   - Update internal deps: `gpui.workspace = true` → `gpui-unofficial = "X.Y.Z"`
   - Set repository/homepage to gpui-unofficial repo
   - Remove `publish = false`
4. **Patch source code:**
   - `use gpui::` → `use gpui_unofficial::`
   - Remove inspector feature from gpui-macros
5. **Write metadata** - Record source tag, SHA, transform date
6. **Validate** - `cargo check --workspace`

## CI/CD Pipeline

### sync.yml
```yaml
on:
  schedule:
    - cron: '0 */6 * * *'  # Every 6 hours
  workflow_dispatch:

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - name: Get latest zed tag
        id: zed
        run: |
          TAG=$(gh api repos/zed-industries/zed/releases/latest --jq .tag_name)
          echo "tag=$TAG" >> $GITHUB_OUTPUT
      
      - name: Check if already published
        id: check
        run: |
          # Compare against last published version
          # If new, trigger transform workflow
```

### transform.yml
```yaml
on:
  workflow_call:
    inputs:
      zed_tag:
        required: true
        type: string

jobs:
  transform:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      
      - name: Transform
        run: cargo xtask transform --zed-tag ${{ inputs.zed_tag }}
      
      - name: Build
        id: build
        run: cargo build --workspace --all-features
        continue-on-error: true
      
      - name: Test
        if: steps.build.outcome == 'success'
        run: cargo test --workspace
      
      - name: Publish
        if: steps.build.outcome == 'success'
        run: cargo xtask publish
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
      
      - name: Dispatch agent on failure
        if: steps.build.outcome == 'failure'
        uses: github/gh-aw@v1
        with:
          workflow: fix-build
          inputs: |
            zed_tag: ${{ inputs.zed_tag }}
            error_log: ${{ steps.build.outputs.stderr }}
```

## Agent Fix Workflow

Defined in `.github/agents/fix-build.md`, compiled with `gh aw compile`.

**Context provided to agent:**
- Build/test error logs
- Transform diff from last successful release
- Upstream zed changelog since last release
- History of past fixes

**Agent capabilities:**
- Edit files in `crates/`
- Add persistent patches to `xtask/patches/`
- Run cargo build/test/clippy
- Commit and push

**Guardrails:**
- Max 3 attempts per release
- Cannot modify workflows or publish directly
- Each attempt is a separate commit

**Failure mode:**
- Opens GitHub issue with error logs and agent attempts
- Humans fix manually, merge triggers retry

## Versioning Strategy

Version numbers track zed releases:
- Zed `v0.185.0` → `gpui-unofficial` `0.185.0`
- Build metadata includes zed commit SHA: `0.185.0+zed.abc1234`

## Usage

Once published, users can depend on:

```toml
[dependencies]
gpui-unofficial = "0.185"
# Or with platform selection:
gpui-platform-unofficial = { version = "0.185", features = ["macos"] }
```

## Bootstrap Steps

1. Create GitHub repo `gpui-unofficial/gpui-unofficial`
2. Set up workspace structure and xtask
3. Write transform logic
4. Write agent workflow markdown
5. Run `gh aw compile`
6. Add `CARGO_REGISTRY_TOKEN` secret
7. Manual first run to validate
8. Enable cron schedule

## Open Questions

- Should we also publish `reqwest-client-unofficial` or keep it internal?
- Do we need Windows CI runners for full platform coverage?
- Should failed releases block subsequent releases or run independently?
