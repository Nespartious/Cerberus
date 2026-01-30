# Development Environment Setup

**Cross-Platform Development: Windows Host → Ubuntu VM Deployment**
## 📖 User Story

```
As a developer working on Windows
I want to develop code locally but test on Ubuntu VM
So that I can contribute effectively without running Linux as my primary OS

Acceptance Criteria:
- VS Code Remote-SSH setup documented with step-by-step guide
- Cross-platform file sync options explained (Git, rsync, shared folders)
- Troubleshooting for common issues (line endings, permissions, compilation)
- Clear explanation of what works on Windows vs requires Ubuntu
- Alternative approaches documented (WSL2, Docker)
```
---

## 📋 Overview

Cerberus is designed for **Linux deployment** (Ubuntu/Debian), but you're developing on **Windows**. This document addresses the development workflow, toolchain setup, and testing strategy for cross-platform development.

### Development Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  WINDOWS DEVELOPMENT MACHINE (Primary Workstation)              │
├─────────────────────────────────────────────────────────────────┤
│  • VS Code + GitHub Copilot                                     │
│  • Git (Nespartious account)                                    │
│  • Rust toolchain (for local development/testing)               │
│  • Documentation editing (.md files)                            │
│  • Configuration file creation                                  │
│                                                                 │
│  ✅ Can Develop Locally:                                        │
│     - Rust code (Fortify application)                           │
│     - Unit tests (cargo test)                                   │
│     - Documentation                                             │
│     - Configuration templates                                   │
│                                                                 │
│  ❌ Cannot Run Locally:                                         │
│     - HAProxy (Linux-only, uses epoll)                          │
│     - Nginx (Windows build exists but differs from Linux)       │
│     - Tor with PoW defenses (Linux-optimized)                   │
│     - Full integration testing                                  │
└─────────────────────────────────────────────────────────────────┘
                            ↓ (git push, file sync, SSH)
┌─────────────────────────────────────────────────────────────────┐
│  UBUNTU VM (Testing/Deployment Target)                          │
├─────────────────────────────────────────────────────────────────┤
│  • Ubuntu 24.04 LTS (fresh install)                             │
│  • Full Cerberus stack deployment                               │
│  • Integration testing                                          │
│  • Performance benchmarking                                     │
│  • Production-like environment                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🛠️ Windows Development Setup

### 1. Core Tools (Already Installed)

✅ **Git for Windows**
- Configured for Nespartious account
- GitHub CLI (`gh`) for repository management

✅ **VS Code**
- GitHub Copilot (JaredCH account)
- Git integration (Nespartious account)

### 2. Rust Toolchain (For Fortify Development)

**Install Rust on Windows:**
```powershell
# Download and run rustup-init.exe
# https://www.rust-lang.org/tools/install

# Or via winget
winget install Rustlang.Rustup

# Verify installation
rustc --version  # Should show 1.82.0 or newer
cargo --version
```

**VS Code Rust Extensions:**
```powershell
# Install via VS Code or CLI
code --install-extension rust-lang.rust-analyzer
code --install-extension serayuzgur.crates
code --install-extension vadimcn.vscode-lldb  # Debugger
```

**Configure rust-analyzer:**
```json
// .vscode/settings.json (Windows-specific)
{
  "rust-analyzer.check.command": "clippy",
  "rust-analyzer.cargo.target": "x86_64-unknown-linux-gnu",  // Target Linux
  "rust-analyzer.checkOnSave.allTargets": false,
  "[rust]": {
    "editor.formatOnSave": true,
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  }
}
```

### 3. What You CAN Do on Windows

**✅ Rust Development:**
```powershell
# Navigate to Fortify project (when created)
cd C:\Users\Jared\Cerberus\Cerberus\keeper

# Run unit tests (most will work on Windows)
cargo test

# Check code (clippy lints)
cargo clippy --all-targets --all-features

# Build (may succeed if no Linux-only syscalls)
cargo build

# Format code
cargo fmt
```

**✅ Documentation:**
- Edit all `.md` files in `docs/`
- Preview Markdown in VS Code (Ctrl+Shift+V)
- Commit and push changes

**✅ Configuration Files:**
- Create HAProxy configs (`config/haproxy.cfg`)
- Create Nginx configs (`config/nginx.conf`)
- Create Tor configs (`config/torrc`)
- These are text files, fully editable on Windows

**✅ Scripts (Bash):**
- Write deployment scripts (`scripts/cerberus.sh`)
- Windows can edit `.sh` files (just text)
- **Cannot execute** without WSL/Git Bash

### 4. What You CANNOT Do on Windows

**❌ HAProxy:**
- Uses Linux-specific APIs (epoll, Linux kernel tuning)
- Windows build is outdated and unsupported
- **Solution**: Test in Ubuntu VM only

**❌ Nginx (Production Config):**
- Windows build exists but differs significantly
- Missing modules, different file paths
- **Solution**: Test in Ubuntu VM only

**❌ Tor with PoW:**
- PoW defenses optimized for Linux
- Windows Tor exists but not production-ready for our use case
- **Solution**: Test in Ubuntu VM only

**❌ Full Integration Testing:**
- Requires all three layers running simultaneously
- **Solution**: Automated testing in Ubuntu VM

---

## 🐧 Ubuntu VM Setup

### VM Specifications (Recommended)

**Hypervisor Options:**
- **Hyper-V** (Built into Windows 10/11 Pro)
- **VirtualBox** (Free, cross-platform)
- **VMware Workstation Player** (Free for personal use)

**VM Configuration:**
```
OS: Ubuntu 24.04 LTS Server (or Desktop if you want GUI)
RAM: 4 GB minimum (8 GB recommended for testing)
CPU: 4 cores (enables parallel testing)
Disk: 50 GB (20 GB for OS, 30 GB for logs/data)
Network: Bridged (get own IP for SSH access)
```

### Initial Ubuntu Setup

**1. Install Ubuntu:**
```bash
# After fresh install, update system
sudo apt update && sudo apt upgrade -y

# Install essential build tools
sudo apt install -y build-essential git curl wget vim tmux

# Install Rust (same version as Windows)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify versions match Windows
rustc --version  # Should match Windows: 1.82.0
```

**2. Install Cerberus Dependencies:**
```bash
# Tor (with PoW support)
sudo apt install -y tor

# HAProxy (2.8 LTS)
sudo apt install -y haproxy

# Nginx (1.26 mainline)
sudo apt install -y nginx

# Vanguards (Tor guard protection)
sudo apt install -y python3-pip
pip3 install vanguards

# mkp224o (vanity onion generator, optional)
sudo apt install -y libsodium-dev
git clone https://github.com/cathugger/mkp224o.git /tmp/mkp224o
cd /tmp/mkp224o && ./autogen.sh && ./configure && make -j$(nproc)
sudo mv mkp224o /usr/local/bin/
```

**3. Configure SSH Access from Windows:**
```bash
# On Ubuntu VM
sudo apt install -y openssh-server
sudo systemctl enable ssh
sudo systemctl start ssh

# Get VM IP address
ip addr show | grep inet
# Example output: inet 192.168.1.50/24

# On Windows, test SSH connection
ssh jared@192.168.1.50
```

---

## 🔄 Development Workflow

### Recommended: Git-Based Sync

**Workflow:**
1. **Develop on Windows**: Edit code in VS Code
2. **Commit locally**: `git add . && git commit -m "..."`
3. **Push to GitHub**: `git push origin main`
4. **Pull on Ubuntu VM**: SSH into VM, `git pull origin main`
5. **Test on Ubuntu**: Run deployment scripts, integration tests
6. **Iterate**: Fix bugs on Windows, repeat

**Example Session:**
```powershell
# On Windows
cd C:\Users\Jared\Cerberus\Cerberus

# Make changes to Rust code
code keeper/src/main.rs

# Test locally (unit tests)
cd keeper
cargo test

# Commit and push
git add .
git commit -m "Add circuit reputation tracking"
git push origin main
```

```bash
# On Ubuntu VM (SSH session)
cd ~/cerberus-deploy
git pull origin main

# Build Fortify
cd keeper
cargo build --release

# Run integration tests
cd ..
./scripts/test-integration.sh

# Check logs
journalctl -u cerberus-fortify -f
```

---

### Alternative 1: VS Code Remote - SSH ⭐ Recommended

**Best of Both Worlds**: Develop on Windows, code executes on Ubuntu VM.

**Setup:**
```powershell
# On Windows, install VS Code extension
code --install-extension ms-vscode-remote.remote-ssh

# Configure SSH connection
# In VS Code: Ctrl+Shift+P → "Remote-SSH: Connect to Host"
# Enter: jared@192.168.1.50

# VS Code will install server components on Ubuntu VM
# You can now edit files directly on VM with Windows UI
```

**Benefits:**
- ✅ Edit files on Ubuntu VM with Windows VS Code UI
- ✅ Rust-analyzer runs on Linux (accurate compilation)
- ✅ Integrated terminal runs bash commands on VM
- ✅ No file sync needed (direct editing)
- ✅ GitHub Copilot works seamlessly

**Workflow with Remote-SSH:**
```
1. Open VS Code on Windows
2. Connect to Ubuntu VM via Remote-SSH
3. Open folder: /home/jared/cerberus
4. Edit code, run tests, deploy - all on VM
5. Commit/push from VM terminal: git push origin main
```

---

### Alternative 2: WSL2 (Windows Subsystem for Linux)

**Turn Windows into a Linux Dev Machine:**

**Pros:**
- ✅ Full Ubuntu environment inside Windows
- ✅ No VM overhead (near-native performance)
- ✅ Seamless file access (Windows <-> WSL2)
- ✅ Can run Docker containers

**Cons:**
- ⚠️ Networking differs from production (NAT, no bridge)
- ⚠️ Not identical to Ubuntu VM (kernel differences)
- ⚠️ Tor performance may differ

**Setup:**
```powershell
# On Windows (as Administrator)
wsl --install

# Install Ubuntu 24.04
wsl --install -d Ubuntu-24.04

# Launch Ubuntu
wsl

# Inside WSL2, install Cerberus dependencies
sudo apt update
sudo apt install -y tor haproxy nginx build-essential git

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**VS Code Integration:**
```powershell
# Install WSL extension
code --install-extension ms-vscode-remote.remote-wsl

# Open project in WSL
cd C:\Users\Jared\Cerberus\Cerberus
wsl
# Inside WSL: code .
# VS Code will connect to WSL environment
```

**Recommendation:**
- **Use WSL2 for development** (fast, convenient)
- **Use Ubuntu VM for final testing** (production-like)

---

### Alternative 3: Docker (Cross-Platform Builds)

**Build Linux binaries on Windows using Docker:**

```powershell
# On Windows, install Docker Desktop
winget install Docker.DockerDesktop

# Create Dockerfile for Rust cross-compilation
# See below for example
```

**Example Dockerfile:**
```dockerfile
# Dockerfile (in project root)
FROM ubuntu:24.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    git \
    haproxy \
    nginx \
    tor \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Set working directory
WORKDIR /cerberus

# Copy project files
COPY . .

# Build Fortify
RUN cd keeper && cargo build --release

# Expose ports (for testing)
EXPOSE 10000 10001 10002

CMD ["/bin/bash"]
```

**Build and Test:**
```powershell
# On Windows
docker build -t cerberus:dev .

# Run container
docker run -it --rm cerberus:dev

# Inside container (Linux environment)
./scripts/cerberus.sh deploy
```

---

## 🧪 Testing Strategy

### 1. Unit Tests (Windows + Ubuntu)

**Run on Windows:**
```powershell
cd C:\Users\Jared\Cerberus\Cerberus\keeper
cargo test

# Most unit tests should work cross-platform
# Failures indicate Linux-specific code
```

**Run on Ubuntu VM:**
```bash
cd ~/cerberus-deploy/keeper
cargo test --release

# All tests should pass here
```

### 2. Integration Tests (Ubuntu VM Only)

**Automated Test Suite:**
```bash
# scripts/test-integration.sh
#!/bin/bash

# Deploy full stack
./scripts/cerberus.sh deploy

# Wait for services to start
sleep 10

# Test Tor connectivity
curl --socks5-hostname 127.0.0.1:9050 http://$(cat /var/lib/tor/cerberus/hostname)

# Test HAProxy stats
echo "show info" | socat stdio /run/haproxy/admin.sock

# Test CAPTCHA generation
curl http://127.0.0.1:10002/captcha/generate

# Cleanup
./scripts/cerberus.sh stop
```

### 3. Load Testing (Ubuntu VM)

**Simulate DDoS Attack:**
```bash
# Install load testing tools
sudo apt install -y apache2-utils siege

# Generate 1000 requests from 100 concurrent connections
ab -n 1000 -c 100 http://127.0.0.1:10000/

# Monitor HAProxy stick tables
watch -n 1 'echo "show table cerberus_circuits" | socat stdio /run/haproxy/admin.sock'
```

---

## 📁 File Synchronization Options

### Option 1: Git (Recommended for Code)

**Pros:**
- ✅ Version control built-in
- ✅ Works with GitHub workflow
- ✅ Selective sync (only committed files)

**Cons:**
- ❌ Requires commit/push/pull cycle
- ❌ Not instant (manual sync)

### Option 2: Shared Folder (VM Host Share)

**VirtualBox Shared Folder:**
```bash
# On Ubuntu VM
sudo apt install -y virtualbox-guest-utils
sudo mount -t vboxsf cerberus /mnt/windows-share

# Edit files on Windows, instantly available on VM
cd /mnt/windows-share
./scripts/cerberus.sh deploy
```

**Hyper-V Enhanced Session:**
```powershell
# On Windows (as Administrator)
Set-VM -VMName "Ubuntu-Cerberus" -EnhancedSessionTransportType HvSocket

# Inside Ubuntu VM, files accessible via /mnt/c/Users/Jared/...
```

**Cons:**
- ⚠️ File permissions may break (Windows NTFS → Linux ext4)
- ⚠️ Line endings (CRLF vs LF) can cause issues

### Option 3: rsync Over SSH

**Automated Sync Script:**
```powershell
# sync-to-vm.ps1 (Windows PowerShell)
$VM_IP = "192.168.1.50"
$VM_USER = "jared"
$LOCAL_PATH = "C:\Users\Jared\Cerberus\Cerberus"
$REMOTE_PATH = "/home/jared/cerberus-deploy"

# Use WSL2 to run rsync (or install rsync for Windows)
wsl rsync -avz --exclude 'target' --exclude '.git' `
    "$LOCAL_PATH/" "$VM_USER@${VM_IP}:$REMOTE_PATH/"

Write-Host "Synced to Ubuntu VM"
```

**Run after every change:**
```powershell
.\sync-to-vm.ps1
ssh jared@192.168.1.50 "cd cerberus-deploy && ./scripts/cerberus.sh restart"
```

### Option 4: VS Code Remote-SSH (No Sync Needed)

**Directly edit files on Ubuntu VM** - See "Alternative 1" above.

---

## 🎯 Recommended Setup (Pragmatic Approach)

### Day-to-Day Development

**Primary: VS Code Remote-SSH to Ubuntu VM** ⭐
1. Keep Ubuntu VM running 24/7 (or start when needed)
2. Open VS Code on Windows
3. Connect to Ubuntu VM via Remote-SSH
4. Edit files directly on VM (no sync needed)
5. Run tests in integrated terminal (on VM)
6. Commit/push from VM terminal

**Benefits:**
- ✅ One environment (no sync issues)
- ✅ Rust compiles for Linux target
- ✅ Full access to HAProxy/Nginx/Tor
- ✅ Windows UI with Linux execution

### Documentation Work

**Local Windows Editing:**
1. Edit `.md` files in `docs/` on Windows
2. Preview in VS Code (Ctrl+Shift+V)
3. Commit and push from Windows
4. No need for VM

### Quick Tests

**Use WSL2 for Rapid Iteration:**
1. Open WSL2 Ubuntu terminal
2. Navigate to project: `cd /mnt/c/Users/Jared/Cerberus/Cerberus`
3. Run unit tests: `cargo test`
4. Faster than VM boot

### Final Validation

**Ubuntu VM for Production-Like Testing:**
1. Deploy full Cerberus stack
2. Run integration tests
3. Performance benchmarking
4. Validate before releases

---

## ⚙️ Configuration Management

### Separate Configs for Dev vs Production

**Directory Structure:**
```
config/
├── dev/                    # Development configs (permissive)
│   ├── haproxy.cfg
│   ├── nginx.conf
│   └── torrc
├── prod/                   # Production configs (hardened)
│   ├── haproxy.cfg
│   ├── nginx.conf
│   └── torrc
└── cerberus.conf.example   # User-editable settings
```

**Environment Variable:**
```bash
# In cerberus.sh
CERBERUS_ENV="${CERBERUS_ENV:-dev}"  # Default to dev

if [ "$CERBERUS_ENV" = "prod" ]; then
    CONFIG_DIR="/etc/cerberus/prod"
else
    CONFIG_DIR="./config/dev"
fi
```

---

## 🐛 Troubleshooting Cross-Platform Issues

### Issue 1: Line Endings (CRLF vs LF)

**Problem:** Scripts fail on Linux with `^M` errors (Windows CRLF endings)

**Solution 1: Git Auto-Convert**
```bash
# .gitattributes (in project root)
* text=auto
*.sh text eol=lf
*.conf text eol=lf
*.md text eol=lf
```

**Solution 2: Manual Conversion**
```bash
# On Ubuntu VM, if script has wrong endings
dos2unix scripts/cerberus.sh

# Or install dos2unix
sudo apt install -y dos2unix
find . -name "*.sh" -exec dos2unix {} \;
```

### Issue 2: File Permissions Lost

**Problem:** Scripts not executable after Windows → Linux transfer

**Solution:**
```bash
# On Ubuntu VM, make scripts executable
chmod +x scripts/*.sh

# Or in Git
git update-index --chmod=+x scripts/cerberus.sh
git commit -m "Make cerberus.sh executable"
```

### Issue 3: Rust Compilation Errors (Windows → Linux)

**Problem:** Code compiles on Windows but fails on Linux (or vice versa)

**Common Causes:**
- Path separators (`\` vs `/`)
- Linux-specific syscalls (e.g., `libc::epoll_wait`)
- Dependencies with platform-specific features

**Solution:**
```rust
// Use platform-agnostic paths
use std::path::PathBuf;

let config_path = PathBuf::from("config").join("cerberus.conf");

// Conditional compilation
#[cfg(target_os = "linux")]
fn linux_specific_code() { /* ... */ }

#[cfg(target_os = "windows")]
fn windows_specific_code() { /* ... */ }
```

### Issue 4: Tor/HAProxy/Nginx Not Starting

**Problem:** Services fail to start with permission errors

**Solution:**
```bash
# Check service users exist
id debian-tor
id haproxy
id www-data

# Fix ownership
sudo chown -R debian-tor:debian-tor /var/lib/tor/
sudo chown -R haproxy:haproxy /run/haproxy/
sudo chown -R www-data:www-data /var/www/
```

---

## 📊 Workflow Comparison Table

| Approach | Ease of Setup | Performance | Production Parity | Recommendation |
|----------|---------------|-------------|-------------------|----------------|
| **Git Sync** | ⭐⭐⭐⭐⭐ Easy | ⭐⭐⭐ Good | ⭐⭐⭐⭐⭐ Perfect | Good for team collab |
| **VS Code Remote-SSH** | ⭐⭐⭐⭐ Moderate | ⭐⭐⭐⭐ Great | ⭐⭐⭐⭐⭐ Perfect | ⭐ **Best overall** |
| **WSL2** | ⭐⭐⭐⭐ Moderate | ⭐⭐⭐⭐⭐ Excellent | ⭐⭐⭐ Good | Fast iteration |
| **Docker** | ⭐⭐⭐ Complex | ⭐⭐⭐⭐ Great | ⭐⭐⭐⭐ Very Good | CI/CD builds |
| **Shared Folder** | ⭐⭐ Tricky | ⭐⭐ Poor | ⭐⭐ Poor | Not recommended |
| **rsync Script** | ⭐⭐⭐ Moderate | ⭐⭐⭐ Good | ⭐⭐⭐⭐ Great | Manual control |

---

## 📝 Setup Checklist

### Windows Development Machine

- [ ] Install Git for Windows (configured for Nespartious)
- [ ] Install VS Code with GitHub Copilot
- [ ] Install Rust toolchain (`rustup-init.exe`)
- [ ] Install VS Code extensions (rust-analyzer, Remote-SSH)
- [ ] Clone Cerberus repository locally
- [ ] Configure `.vscode/settings.json` for Linux target

### Ubuntu VM

- [ ] Install Ubuntu 24.04 LTS (4GB RAM, 4 CPU cores)
- [ ] Configure bridged networking (get own IP)
- [ ] Install SSH server (`openssh-server`)
- [ ] Install build tools (`build-essential`, `git`, `curl`)
- [ ] Install Rust toolchain (same version as Windows)
- [ ] Install Cerberus dependencies (Tor, HAProxy, Nginx)
- [ ] Clone Cerberus repository: `git clone https://github.com/Nespartious/Cerberus.git`
- [ ] Test SSH access from Windows: `ssh jared@192.168.1.50`

### VS Code Remote-SSH (Recommended)

- [ ] Install Remote-SSH extension in VS Code
- [ ] Configure SSH connection to Ubuntu VM
- [ ] Connect to VM and open `/home/jared/cerberus` folder
- [ ] Verify Rust-analyzer works (check bottom status bar)
- [ ] Test integrated terminal runs bash commands
- [ ] Test Git operations from VS Code (commit/push)

### WSL2 (Optional, for Quick Tests)

- [ ] Enable WSL2: `wsl --install`
- [ ] Install Ubuntu 24.04: `wsl --install -d Ubuntu-24.04`
- [ ] Install dependencies inside WSL2
- [ ] Install Remote-WSL extension in VS Code
- [ ] Test opening project in WSL: `code .` from WSL terminal

---

## 🔗 Useful Links

- **VS Code Remote-SSH**: https://code.visualstudio.com/docs/remote/ssh
- **WSL2 Setup**: https://learn.microsoft.com/en-us/windows/wsl/install
- **Rust Cross-Compilation**: https://rust-lang.github.io/rustup/cross-compilation.html
- **VirtualBox**: https://www.virtualbox.org/
- **Hyper-V Quick Create**: https://learn.microsoft.com/en-us/virtualization/hyper-v-on-windows/

---

**Status**: 📝 Design Document (Setup required before Sprint 2 implementation)
