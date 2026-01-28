# Cerberus Project - Folder Structure Scaffold

## Overview
This document outlines the complete folder structure for the Cerberus high-assurance Tor ingress system. The structure is organized to support development, deployment, configuration management, and operational security.

---

## Root Directory Structure

```
Cerberus/
├── docs/                          # Documentation
│   ├── CERBERUS_MASTER_ARCH.md   # Master architecture document (existing)
│   ├── scaffold.md               # This file
│   ├── deployment-guide.md       # Deployment instructions
│   ├── security-hardening.md     # Security best practices
│   └── troubleshooting.md        # Common issues and solutions
│
├── scripts/                       # Deployment and maintenance scripts
│   ├── cerberus.sh               # Main orchestrator script
│   ├── cerberus.conf             # User configuration file
│   ├── install/                  # Installation modules
│   │   ├── detect-os.sh
│   │   ├── install-deps.sh
│   │   └── system-hardening.sh
│   ├── config/                   # Configuration generators
│   │   ├── generate-tor.sh
│   │   ├── generate-haproxy.sh
│   │   └── generate-nginx.sh
│   ├── utils/                    # Utility scripts
│   │   ├── health-check.sh
│   │   ├── log-analyzer.sh
│   │   └── backup-config.sh
│   └── audit/                    # Security audit scripts
│       ├── audit-system.sh
│       └── verify-setup.sh
│
├── config/                        # Configuration templates
│   ├── templates/
│   │   ├── torrc.template
│   │   ├── haproxy.cfg.template
│   │   ├── nginx.conf.template
│   │   └── vanguards.conf.template
│   └── examples/
│       ├── cerberus.conf.example
│       └── ddos-profiles/
│           ├── low-sensitivity.conf
│           ├── medium-sensitivity.conf
│           └── high-sensitivity.conf
│
├── keeper/                        # The Keeper - Rust application (Layer 3)
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── captcha/              # CAPTCHA verification module
│   │   │   ├── mod.rs
│   │   │   ├── generator.rs
│   │   │   └── validator.rs
│   │   ├── haproxy/              # HAProxy stick table integration
│   │   │   ├── mod.rs
│   │   │   └── client.rs
│   │   ├── swarm/                # Swarm state management
│   │   │   ├── mod.rs
│   │   │   └── state.rs
│   │   ├── api/                  # HTTP API handlers
│   │   │   ├── mod.rs
│   │   │   └── handlers.rs
│   │   └── config/               # Configuration management
│   │       ├── mod.rs
│   │       └── settings.rs
│   ├── tests/
│   │   ├── integration/
│   │   └── unit/
│   └── benches/                  # Performance benchmarks
│
├── static/                        # Static assets (Layer 2 - Nginx)
│   ├── captcha.html              # Static CAPTCHA gate page
│   ├── css/
│   │   └── captcha.css
│   ├── js/
│   │   └── captcha.js
│   └── images/
│       └── captcha-background.png
│
├── tests/                         # Integration and system tests
│   ├── integration/
│   │   ├── test-tor-connectivity.sh
│   │   ├── test-haproxy-circuit-id.sh
│   │   └── test-full-pipeline.sh
│   ├── load-testing/
│   │   ├── ddos-simulation.py
│   │   └── load-test-config.yaml
│   └── security/
│       ├── penetration-test.sh
│       └── vulnerability-scan.sh
│
├── deployment/                    # Deployment configurations
│   ├── systemd/                  # Service files (Ubuntu/Debian)
│   │   ├── cerberus-tor.service
│   │   ├── cerberus-haproxy.service
│   │   ├── cerberus-nginx.service
│   │   ├── cerberus-keeper.service
│   │   └── cerberus-vanguards.service
│   ├── docker/                   # Docker deployment (Recommended)
│   │   ├── Dockerfile.tor
│   │   ├── Dockerfile.haproxy
│   │   ├── Dockerfile.nginx
│   │   ├── Dockerfile.keeper
│   │   └── docker-compose.yml
│   └── ansible/                  # Ansible playbooks (Advanced)
│       ├── playbook.yml
│       └── roles/
│
├── monitoring/                    # Monitoring and logging
│   ├── dashboards/
│   │   └── grafana-dashboard.json
│   ├── alerts/
│   │   └── alert-rules.yml
│   └── log-parsers/
│       ├── haproxy-parser.py
│       └── nginx-parser.py
│
├── mock-target/                   # Sprint 1: Mock target service
│   ├── server.py                 # Simple Python HTTP server
│   └── index.html
│
└── runtime/                       # Runtime data (gitignored)
    ├── data/                     # Application data
    ├── logs/                     # Centralized logs
    │   ├── tor/
    │   ├── haproxy/
    │   ├── nginx/
    │   └── keeper/
    ├── keys/                     # Tor keys & secrets
    └── state/                    # Swarm state and session data
```

---

## Key Design Principles

### 1. Separation of Concerns
- **docs/**: All documentation in one place
- **scripts/**: Modular deployment automation
- **config/**: Centralized configuration management
- **keeper/**: Isolated Rust application following cargo conventions
- **static/**: Pure frontend assets served directly by Nginx

### 2. Security by Design
- **runtime/**: All dynamic data isolated and gitignored
- **keys/**: Separate directory for sensitive cryptographic material
- **tests/security/**: Dedicated security testing suite

### 3. Operational Readiness
- **deployment/**: Production-ready service configurations
- **monitoring/**: Observability from day one
- **scripts/utils/**: Maintenance and troubleshooting tools

### 4. Sprint-Based Development
- **mock-target/**: Supports Sprint 1 pipeline verification
- **keeper/**: Prepared for Sprint 2+ Rust development
- Modular structure allows incremental feature addition

---

## Implementation Priority (Sprint 1)

### Phase 1: Foundation
1. Create `scripts/cerberus.sh` and `scripts/cerberus.conf`
2. Set up `config/templates/` with torrc, haproxy, nginx templates
3. Implement `scripts/install/` modules

### Phase 2: Static Assets
4. Create `static/captcha.html` (basic placeholder)
5. Set up `mock-target/server.py`

### Phase 3: Deployment
6. Create `deployment/systemd/` service files
7. Implement `scripts/config/` generators

### Phase 4: Verification
8. Implement `tests/integration/test-full-pipeline.sh`
9. Create `scripts/utils/health-check.sh`

---

## Git Ignore Recommendations

```gitignore
# Runtime data
runtime/
/data/
/logs/
/keys/
/state/

# Rust build artifacts
keeper/target/
keeper/Cargo.lock

# User configuration
cerberus.conf
!cerberus.conf.example

# Test artifacts
tests/**/*.log
tests/**/results/

# OS specific
.DS_Store
Thumbs.db
*~
```

---

## Next Steps

1. **Review this scaffold** with the team/user
2. **Create directory structure** using mkdir commands
3. **Initialize Git** with appropriate .gitignore
4. **Begin Sprint 1 implementation** following the priority order
5. **Set up CI/CD pipeline** (optional, post-MVP)

---

## Notes

- The structure supports both bare-metal and containerized deployments
- Ansible/Docker directories are optional for advanced users
- The Keeper application follows standard Rust project conventions
- All secrets and runtime data are segregated from version control
- Modular design allows easy extension for future features
