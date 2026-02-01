#!/usr/bin/env python3
"""
Cerberus Dashboard Server
Serves the deployment dashboard and provides real-time log streaming.
"""

import http.server
import socketserver
import json
import subprocess
import threading
import queue
import os
import time
import re
from pathlib import Path
from urllib.parse import urlparse
from datetime import datetime

# Configuration
PORT = 9999
DASHBOARD_DIR = Path(__file__).parent / "dashboard"
LOG_QUEUE = queue.Queue(maxsize=1000)
START_TIME = time.time()

# Stats tracking
STATS = {
    "requests": 0,
    "blocked": 0,
    "captchas": 0
}

class DashboardHandler(http.server.SimpleHTTPRequestHandler):
    """HTTP handler for the dashboard API and static files."""
    
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(DASHBOARD_DIR), **kwargs)
    
    def log_message(self, format, *args):
        """Suppress default logging."""
        pass
    
    def do_GET(self):
        """Handle GET requests."""
        path = urlparse(self.path).path
        
        if path == "/api/status":
            self.send_json(self.get_status())
        elif path == "/api/logs/stream":
            self.stream_logs()
        elif path == "/api/logs/history":
            self.send_json(self.get_log_history())
        else:
            # Serve static files
            if path == "/":
                self.path = "/index.html"
            super().do_GET()
    
    def send_json(self, data):
        """Send JSON response."""
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())
    
    def get_status(self):
        """Get current system status."""
        services = {}
        
        for svc in ["fortify", "tor", "haproxy", "nginx", "redis-server"]:
            try:
                result = subprocess.run(
                    ["systemctl", "is-active", svc],
                    capture_output=True, text=True, timeout=5
                )
                status = result.stdout.strip()
                svc_name = svc.replace("-server", "")
                services[svc_name] = "running" if status == "active" else "stopped"
            except Exception:
                services[svc.replace("-server", "")] = "unknown"
        
        # Get onion addresses
        mirror_onion = None
        backend_onion = "sigilahzwq5u34gdh2bl3ymokyc7kobika55kyhztsucdoub73hz7qid.onion"
        
        try:
            with open("/var/lib/tor/cerberus_hs/hostname", "r") as f:
                mirror_onion = f.read().strip()
        except Exception:
            pass
        
        return {
            "services": services,
            "mirror_onion": mirror_onion,
            "backend_onion": backend_onion,
            "stats": STATS,
            "start_time": START_TIME * 1000  # JS timestamp
        }
    
    def stream_logs(self):
        """Stream logs via Server-Sent Events."""
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        
        # Send initial connection message
        self.send_sse({"time": datetime.now().strftime("%H:%M:%S"), 
                       "level": "info", "source": "dashboard", 
                       "message": "Connected to log stream"})
        
        try:
            while True:
                try:
                    log_entry = LOG_QUEUE.get(timeout=1)
                    self.send_sse(log_entry)
                except queue.Empty:
                    # Send keepalive
                    self.wfile.write(b": keepalive\n\n")
                    self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass
    
    def send_sse(self, data):
        """Send SSE event."""
        self.wfile.write(f"data: {json.dumps(data)}\n\n".encode())
        self.wfile.flush()
    
    def get_log_history(self):
        """Get recent log history."""
        return list(LOG_QUEUE.queue)[-100:]


def parse_log_line(line, source):
    """Parse a log line and extract components."""
    timestamp = datetime.now().strftime("%H:%M:%S")
    level = "info"
    message = line.strip()
    
    # Try to extract timestamp
    time_match = re.search(r'(\d{2}:\d{2}:\d{2})', line)
    if time_match:
        timestamp = time_match.group(1)
    
    # Detect log level
    line_lower = line.lower()
    if "error" in line_lower or "failed" in line_lower or "fatal" in line_lower:
        level = "error"
    elif "warn" in line_lower or "warning" in line_lower:
        level = "warn"
    elif "debug" in line_lower or "trace" in line_lower:
        level = "debug"
    
    # Update stats based on log content
    global STATS
    if "request" in line_lower:
        STATS["requests"] += 1
    if "blocked" in line_lower or "denied" in line_lower or "reject" in line_lower:
        STATS["blocked"] += 1
    if "captcha" in line_lower:
        STATS["captchas"] += 1
    
    return {
        "time": timestamp,
        "level": level,
        "source": source,
        "message": message
    }


def tail_journalctl(service, source_name):
    """Tail journalctl for a service."""
    try:
        process = subprocess.Popen(
            ["journalctl", "-u", service, "-f", "-n", "0", "--no-pager"],
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True
        )
        
        for line in iter(process.stdout.readline, ''):
            if line.strip():
                entry = parse_log_line(line, source_name)
                try:
                    LOG_QUEUE.put_nowait(entry)
                except queue.Full:
                    try:
                        LOG_QUEUE.get_nowait()
                        LOG_QUEUE.put_nowait(entry)
                    except:
                        pass
    except Exception as e:
        print(f"Error tailing {service}: {e}")


def tail_file(filepath, source_name):
    """Tail a log file."""
    try:
        with open(filepath, 'r') as f:
            f.seek(0, 2)  # Go to end
            while True:
                line = f.readline()
                if line:
                    entry = parse_log_line(line, source_name)
                    try:
                        LOG_QUEUE.put_nowait(entry)
                    except queue.Full:
                        try:
                            LOG_QUEUE.get_nowait()
                            LOG_QUEUE.put_nowait(entry)
                        except:
                            pass
                else:
                    time.sleep(0.1)
    except Exception as e:
        print(f"Error tailing {filepath}: {e}")


def start_log_collectors():
    """Start background threads for log collection."""
    collectors = [
        ("fortify", "fortify"),
        ("tor", "tor"),
        ("haproxy", "haproxy"),
        ("nginx", "nginx"),
        ("redis-server", "redis"),
    ]
    
    for service, name in collectors:
        thread = threading.Thread(target=tail_journalctl, args=(service, name), daemon=True)
        thread.start()
    
    # Also tail nginx access log
    nginx_access = Path("/var/log/nginx/access.log")
    if nginx_access.exists():
        thread = threading.Thread(target=tail_file, args=(str(nginx_access), "nginx"), daemon=True)
        thread.start()


def main():
    """Main entry point."""
    print(f"\n🔱 Cerberus Dashboard Server")
    print(f"{'=' * 40}")
    print(f"Dashboard: http://127.0.0.1:{PORT}/")
    print(f"API:       http://127.0.0.1:{PORT}/api/status")
    print(f"{'=' * 40}\n")
    
    # Start log collectors
    start_log_collectors()
    
    # Add startup log
    LOG_QUEUE.put({
        "time": datetime.now().strftime("%H:%M:%S"),
        "level": "info",
        "source": "dashboard",
        "message": "Dashboard server started"
    })
    
    # Start HTTP server
    with socketserver.TCPServer(("", PORT), DashboardHandler) as httpd:
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nShutting down...")


if __name__ == "__main__":
    main()
