# CI/CD Workflows & Code Review Standards

## 📖 User Story

```
As a developer submitting a pull request
I want automated checks to catch security issues and bugs before merge
So that I maintain code quality and don't introduce vulnerabilities

Acceptance Criteria:
- cargo-audit checks for vulnerable dependencies
- gitleaks scans for accidentally committed secrets
- clippy enforces Rust linting standards
- All unit and integration tests must pass
- Breaking changes detection for config file modifications
```

---

## Overview
This document defines the automated checks, testing requirements, and code review standards for Cerberus. All pull requests must pass these checks before merging to maintain security, stability, and code quality.

---

## Continuous Integration Pipeline

### Workflow Triggers

```yaml
# Run on:
on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main, develop ]
  schedule:
    - cron: '0 2 * * *'  # Daily at 2 AM UTC (dependency audits)
```

---

## Stage 1: Security Audits (Critical)

### 1.1 Dependency Vulnerability Scanning

**Tool**: `cargo audit` (Rust), `npm audit` (if JS exists), `trivy` (Docker)

**Purpose**: Detect known CVEs in dependencies

**Configuration**:
```yaml
security-audit:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
    
    - name: Install cargo-audit
      run: cargo install cargo-audit --locked
    
    - name: Audit Rust dependencies
      run: |
        cd keeper
        cargo audit --deny warnings
    
    - name: Scan Docker images (if using)
      run: |
        docker build -t cerberus-test .
        trivy image --severity HIGH,CRITICAL cerberus-test
```

**Failure Action**: ❌ Block merge, create security issue

---

### 1.2 Secret Detection

**Tool**: `trufflehog`, `gitleaks`, or `detect-secrets`

**Purpose**: Prevent credential leaks

**Configuration**:
```yaml
secret-scan:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
      with:
        fetch-depth: 0  # Full history for secret scanning
    
    - name: Install gitleaks
      run: |
        wget https://github.com/gitleaks/gitleaks/releases/download/v8.21.2/gitleaks_8.21.2_linux_x64.tar.gz
        tar -xzf gitleaks_8.21.2_linux_x64.tar.gz
        sudo mv gitleaks /usr/local/bin/
    
    - name: Scan for secrets
      run: gitleaks detect --source . --verbose --no-git
```

**Patterns Detected**:
- Private keys (RSA, Ed25519)
- API tokens
- Tor onion private keys
- HMAC secrets
- Database credentials

**Failure Action**: ❌ Block merge immediately, rotate secrets

---

### 1.3 SAST (Static Application Security Testing)

**Tool**: `cargo clippy` with security lints

**Purpose**: Detect unsafe code patterns

**Configuration**:
```yaml
sast:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
      with:
        components: clippy
    
    - name: Run Clippy with security lints
      run: |
        cd keeper
        cargo clippy --all-targets --all-features -- \
          -D warnings \
          -D clippy::suspicious \
          -D clippy::panic \
          -D clippy::unwrap_used \
          -D clippy::expect_used \
          -D clippy::todo \
          -D clippy::unimplemented
```

**Blocked Patterns**:
- `unwrap()` without justification
- `panic!()` in production code
- `unsafe` blocks without comments
- SQL string concatenation
- Unvalidated user input

**Failure Action**: ⚠️ Warning for minor issues, ❌ block for critical

---

## Stage 2: Code Quality & Standards

### 2.1 Linting

**Rust**: `cargo fmt` + `cargo clippy`  
**Shell Scripts**: `shellcheck`  
**YAML/JSON**: `yamllint`, `jsonlint`

**Configuration**:
```yaml
lint:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Check Rust formatting
      run: |
        cd keeper
        cargo fmt --all -- --check
    
    - name: Lint Rust code
      run: |
        cd keeper
        cargo clippy --all-targets --all-features -- -D warnings
    
    - name: Lint shell scripts
      run: |
        find scripts -name "*.sh" -exec shellcheck {} \;
    
    - name: Lint YAML files
      run: |
        find . -name "*.yml" -o -name "*.yaml" | xargs yamllint -d relaxed
```

**Failure Action**: ❌ Block merge (formatting is non-negotiable)

---

### 2.2 Code Coverage

**Tool**: `cargo tarpaulin` (Rust), `coverage.py` (Python if used)

**Purpose**: Ensure adequate test coverage

**Configuration**:
```yaml
coverage:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Install tarpaulin
      run: cargo install cargo-tarpaulin
    
    - name: Generate coverage report
      run: |
        cd keeper
        cargo tarpaulin --out Xml --output-dir coverage
    
    - name: Upload to Codecov
      uses: codecov/codecov-action@v4
      with:
        files: keeper/coverage/cobertura.xml
        fail_ci_if_error: false  # Don't block, just report
```

**Thresholds**:
- Critical modules (CAPTCHA, HAProxy client): **≥ 80%**
- Utility modules: **≥ 60%**
- Total project: **≥ 70%**

**Failure Action**: ⚠️ Warning only (doesn't block merge)

---

## Stage 3: Testing

### 3.1 Unit Tests

**Purpose**: Test individual functions/modules

**Configuration**:
```yaml
unit-tests:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Run Rust unit tests
      run: |
        cd keeper
        cargo test --all-features --lib
```

**Requirements**:
- All new functions must have unit tests
- Critical logic (CAPTCHA validation, token signing) must have 100% coverage
- Tests must be deterministic (no flaky tests)

**Failure Action**: ❌ Block merge

---

### 3.2 Integration Tests

**Purpose**: Test component interactions (HAProxy → Nginx → Fortify)

**Configuration**:
```yaml
integration-tests:
  runs-on: ubuntu-latest
  services:
    tor:
      image: thetorproject/tor:latest
    haproxy:
      image: haproxy:2.8-alpine
  
  steps:
    - uses: actions/checkout@v4
    
    - name: Setup test environment
      run: |
        docker-compose -f tests/docker-compose.test.yml up -d
        sleep 10  # Wait for services to start
    
    - name: Run integration tests
      run: |
        ./tests/integration/test-full-pipeline.sh
        ./tests/integration/test-haproxy-circuit-id.sh
        ./tests/integration/test-virtual-queue.sh
    
    - name: Cleanup
      run: docker-compose -f tests/docker-compose.test.yml down
```

**Test Scenarios**:
- ✅ Tor → HAProxy → Nginx → Fortify (happy path)
- ✅ Circuit ID extraction from HAProxy
- ✅ CAPTCHA generation and verification
- ✅ Virtual queue token generation and validation
- ✅ VIP promotion after CAPTCHA success
- ✅ Circuit banning after repeated failures

**Failure Action**: ❌ Block merge

---

### 3.3 End-to-End Tests (Tor Browser)

**Purpose**: Validate user experience in Tor Browser

**Configuration**:
```yaml
e2e-tests:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Install Tor Browser (Selenium)
      run: |
        sudo apt install firefox-esr
        pip install selenium tbselenium
    
    - name: Run E2E tests
      run: |
        python tests/browser/test_captcha_flow.py
        python tests/browser/test_safest_mode.py
```

**Test Cases**:
- ✅ CAPTCHA page loads without JavaScript
- ✅ CAPTCHA form submits successfully
- ✅ Valid solution grants access
- ✅ Invalid solution shows error
- ✅ Virtual queue page auto-refreshes
- ✅ No JavaScript errors in console (Standard mode)

**Failure Action**: ⚠️ Warning (flaky due to Tor network, doesn't block)

---

### 3.4 Load & Stress Testing

**Purpose**: Validate performance under attack scenarios

**Configuration**:
```yaml
load-tests:
  runs-on: ubuntu-latest
  if: github.event_name == 'pull_request' && contains(github.event.pull_request.labels.*.name, 'performance')
  
  steps:
    - uses: actions/checkout@v4
    
    - name: Setup test environment
      run: docker-compose -f tests/docker-compose.test.yml up -d
    
    - name: Run load test (1000 concurrent)
      run: |
        python tests/load-testing/ddos-simulation.py \
          --circuits 1000 \
          --duration 60 \
          --target http://127.0.0.1:10000
    
    - name: Verify performance thresholds
      run: |
        # Check that 95th percentile latency < 500ms
        # Check that error rate < 1%
        python tests/load-testing/analyze_results.py
```

**Performance Targets**:
- 10,000 concurrent connections supported
- 95th percentile latency < 500ms
- Error rate < 1% under normal load
- 0 crashes or panics

**Failure Action**: ⚠️ Warning (manual review required)

---

## Stage 4: Breaking Changes Detection

### 4.1 API Contract Testing

**Tool**: Custom script + `cargo semver-checks`

**Purpose**: Detect breaking API changes

**Configuration**:
```yaml
api-contract:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
      with:
        fetch-depth: 0  # Need git history
    
    - name: Install semver-checks
      run: cargo install cargo-semver-checks
    
    - name: Check for breaking changes
      run: |
        cd keeper
        cargo semver-checks check-release
```

**Breaking Changes Detected**:
- Public API function signature changes
- Struct field removals
- Enum variant changes
- Config file format changes

**Failure Action**: ⚠️ Require explicit version bump (0.1.0 → 0.2.0)

---

### 4.2 Configuration Compatibility

**Tool**: Custom validator script

**Purpose**: Ensure old configs still work

**Configuration**:
```yaml
config-compat:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Test config backward compatibility
      run: |
        ./tests/config-validator.sh tests/fixtures/old-config-v0.1.toml
        ./tests/config-validator.sh tests/fixtures/old-config-v0.2.toml
```

**Test Cases**:
- ✅ Old `cerberus.conf` parses successfully
- ✅ Old `fortify.toml` loads with defaults
- ✅ Migration path documented for removed options

**Failure Action**: ⚠️ Warning + require migration guide

---

## Stage 5: Build & Artifact Generation

### 5.1 Release Build

**Purpose**: Produce production-ready binaries

**Configuration**:
```yaml
build:
  runs-on: ubuntu-latest
  needs: [security-audit, lint, unit-tests, integration-tests]
  
  steps:
    - uses: actions/checkout@v4
    
    - name: Build release binary
      run: |
        cd keeper
        cargo build --release
    
    - name: Strip debug symbols
      run: strip keeper/target/release/fortify
    
    - name: Generate checksum
      run: |
        cd keeper/target/release
        sha256sum fortify > fortify.sha256
    
    - name: Upload artifact
      uses: actions/upload-artifact@v4
      with:
        name: fortify-linux-x64
        path: |
          keeper/target/release/fortify
          keeper/target/release/fortify.sha256
```

**Artifacts**:
- `fortify` (stripped, optimized binary)
- `fortify.sha256` (checksum for verification)

**Failure Action**: ❌ Block if build fails

---

### 5.2 Docker Image Build

**Purpose**: Produce containerized deployment

**Configuration**:
```yaml
docker-build:
  runs-on: ubuntu-latest
  needs: [build]
  
  steps:
    - uses: actions/checkout@v4
    
    - name: Build Docker images
      run: |
        docker build -f deployment/docker/Dockerfile.tor -t cerberus/tor:test .
        docker build -f deployment/docker/Dockerfile.haproxy -t cerberus/haproxy:test .
        docker build -f deployment/docker/Dockerfile.nginx -t cerberus/nginx:test .
        docker build -f deployment/docker/Dockerfile.fortify -t cerberus/fortify:test .
    
    - name: Test Docker Compose
      run: |
        docker-compose -f deployment/docker/docker-compose.yml up -d
        sleep 10
        curl -x socks5h://127.0.0.1:9050 http://$(docker-compose exec tor cat /var/lib/tor/cerberus/hostname)
```

**Failure Action**: ❌ Block merge (Docker must work)

---

## Stage 6: Documentation Validation

### 6.1 Markdown Linting

**Tool**: `markdownlint`

**Configuration**:
```yaml
docs-lint:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Lint markdown
      uses: DavidAnson/markdownlint-cli2-action@v18
      with:
        globs: '**/*.md'
```

**Failure Action**: ⚠️ Warning (doesn't block)

---

### 6.2 Link Checking

**Tool**: `markdown-link-check`

**Purpose**: Detect broken links in documentation

**Configuration**:
```yaml
link-check:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    
    - name: Check links in docs
      uses: gaurav-nelson/github-action-markdown-link-check@v1
      with:
        use-quiet-mode: 'yes'
        config-file: '.markdown-link-check.json'
```

**Failure Action**: ⚠️ Warning (external links may be flaky)

---

## Code Review Requirements

### Mandatory Checklist (PR Template)

```markdown
## Pull Request Checklist

### Security
- [ ] No secrets committed (keys, passwords, tokens)
- [ ] Input validation added for all user inputs
- [ ] No unsafe code without justification
- [ ] Timing attack vulnerabilities checked
- [ ] CAPTCHA/token logic reviewed

### Testing
- [ ] Unit tests added for new code
- [ ] Integration tests pass locally
- [ ] Tested in Tor Browser (Safest mode)
- [ ] No performance regression

### Code Quality
- [ ] Code formatted with `cargo fmt`
- [ ] No clippy warnings
- [ ] Shell scripts pass shellcheck
- [ ] Comments added for complex logic

### Documentation
- [ ] README updated (if user-facing changes)
- [ ] API docs updated (if endpoints changed)
- [ ] CHANGELOG.md updated
- [ ] Migration guide added (if breaking changes)

### Configuration
- [ ] Backward compatible OR migration path documented
- [ ] Example configs updated
- [ ] Environment variables documented

## CI/CD Status
All checks must pass before merge:
- ✅ Security audit
- ✅ Linting
- ✅ Unit tests
- ✅ Integration tests
- ✅ Build artifacts generated
```

---

### Reviewer Responsibilities

#### Required Reviews

| Change Type | Reviewers Required | Approval Threshold |
|-------------|-------------------|-------------------|
| **Security-critical** (CAPTCHA, auth, crypto) | 2 | Both must approve |
| **Architecture** (new layers, major refactor) | 2 | Both must approve |
| **Configuration** (HAProxy, Nginx, Tor) | 1 | Must approve |
| **Documentation only** | 1 | Must approve |
| **Bug fixes** | 1 | Must approve |
| **Tests only** | 1 | Must approve |

#### Review Focus Areas

**Security Review**:
- [ ] No hardcoded secrets
- [ ] Input validation comprehensive
- [ ] No timing attack vulnerabilities
- [ ] Error messages don't leak info
- [ ] Crypto uses constant-time operations

**Performance Review**:
- [ ] No blocking I/O in async context
- [ ] Database queries optimized
- [ ] No unbounded memory allocations
- [ ] Caching implemented where appropriate

**Maintainability Review**:
- [ ] Code is self-documenting OR well-commented
- [ ] Functions have single responsibility
- [ ] Error handling is comprehensive
- [ ] Tests are readable and maintainable

---

## Automated Merge Rules

### Auto-Merge Allowed (Dependabot)

```yaml
# .github/workflows/auto-merge-dependabot.yml
name: Auto-merge Dependabot

on:
  pull_request:
    types: [opened, synchronize]

jobs:
  auto-merge:
    if: github.actor == 'dependabot[bot]'
    runs-on: ubuntu-latest
    steps:
      - name: Check if patch update
        id: check
        run: |
          # Only auto-merge patch updates (1.2.3 → 1.2.4)
          # Not minor (1.2.x → 1.3.x) or major (1.x → 2.x)
          echo "is_patch=true" >> $GITHUB_OUTPUT
      
      - name: Auto-merge
        if: steps.check.outputs.is_patch == 'true'
        run: gh pr merge --auto --squash "$PR_URL"
        env:
          PR_URL: ${{ github.event.pull_request.html_url }}
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

**Conditions**:
- Patch version updates only (1.2.3 → 1.2.4)
- All CI checks pass
- No security vulnerabilities introduced

---

## Monitoring & Alerts

### Daily Scheduled Checks

```yaml
name: Daily Security Audit

on:
  schedule:
    - cron: '0 2 * * *'  # 2 AM UTC daily

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Audit dependencies
        run: |
          cd keeper
          cargo audit --deny warnings || echo "AUDIT_FAILED=true" >> $GITHUB_ENV
      
      - name: Notify on Slack
        if: env.AUDIT_FAILED == 'true'
        uses: slackapi/slack-github-action@v1
        with:
          payload: |
            {
              "text": "🚨 Cerberus dependency audit failed! Check GitHub Actions."
            }
```

---

## Summary of CI/CD Stages

| Stage | Tools | Failure Action | Time |
|-------|-------|---------------|------|
| **Security Audit** | cargo-audit, gitleaks, clippy | ❌ Block | 2 min |
| **Linting** | rustfmt, shellcheck, yamllint | ❌ Block | 1 min |
| **Unit Tests** | cargo test | ❌ Block | 3 min |
| **Integration Tests** | Docker Compose + scripts | ❌ Block | 5 min |
| **E2E Tests** | Tor Browser + Selenium | ⚠️ Warning | 10 min |
| **Load Tests** | Custom Python scripts | ⚠️ Warning | 5 min |
| **Breaking Changes** | semver-checks, config validator | ⚠️ Warning | 2 min |
| **Build** | cargo build --release | ❌ Block | 5 min |
| **Docker Build** | docker build + docker-compose | ❌ Block | 8 min |
| **Docs** | markdownlint, link-check | ⚠️ Warning | 2 min |

**Total Pipeline Time**: ~20-30 minutes (parallel execution)

---

## Recommended GitHub Branch Protection Rules

```yaml
# Settings → Branches → Branch protection rules
Branch: main
Protection rules:
  ✅ Require pull request reviews before merging (1 approval)
  ✅ Dismiss stale pull request approvals when new commits are pushed
  ✅ Require status checks to pass before merging:
      - security-audit
      - lint
      - unit-tests
      - integration-tests
      - build
      - docker-build
  ✅ Require branches to be up to date before merging
  ✅ Require signed commits
  ✅ Include administrators (no bypass)
  ✅ Restrict who can push (maintainers only)
```

---

## Future Enhancements

1. **Mutation Testing**: Use `cargo-mutants` to verify test effectiveness
2. **Fuzz Testing**: AFL.rs for input fuzzing (CAPTCHA parser, token validator)
3. **Performance Regression Detection**: Benchmark PRs against main branch
4. **Automated Dependency Updates**: Renovate bot for smarter updates
5. **SBOM Generation**: Software Bill of Materials for supply chain transparency
