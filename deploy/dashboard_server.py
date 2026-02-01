#!/usr/bin/env python3
"""
Cerberus Dashboard Server - Simplified
Serves dashboard and provides status API.
"""

import http.server
import socketserver
import json
import subprocess
import os
import time
from pathlib import Path
from urllib.parse import urlparse

PORT = 9999
DASHBOARD_DIR = Path(__file__).parent / "dashboard"
START_TIME = time.time()

class DashboardHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(DASHBOARD_DIR), **kwargs)
    
    def log_message(self, format, *args):
        print(f"[{time.strftime('%H:%M:%S')}] {format % args}")
    
    def do_GET(self):
        path = urlparse(self.path).path
        
        if path == "/api/status":
            self.handle_status()
        elif path == "/api/logs/stream":
            self.handle_log_stream()
        else:
            if path == "/":
                self.path = "/index.html"
            super().do_GET()
    
    def handle_status(self):
        """Get service status."""
        try:
            services = {}
            for svc in ["fortify", "tor", "haproxy", "nginx", "redis-server"]:
                try:
                    result = subprocess.run(
                        ["systemctl", "is-active", svc],
                        capture_output=True, 
                        text=True, 
                        timeout=2
                    )
                    status = result.stdout.strip()
                    svc_name = svc.replace("-server", "")
                    services[svc_name] = "running" if status == "active" else "stopped"
                except subprocess.TimeoutExpired:
                    services[svc.replace("-server", "")] = "timeout"
                except Exception as e:
                    services[svc.replace("-server", "")] = "error"
            
            # Get onion addresses
            mirror_onion = None
            try:
                with open("/var/lib/tor/cerberus_hs/hostname", "r") as f:
                    mirror_onion = f.read().strip()
            except:
                pass
            
            response = {
                "services": services,
                "mirror_onion": mirror_onion,
                "backend_onion": "sigilahzwq5u34gdh2bl3ymokyc7kobika55kyhztsucdoub73hz7qid.onion",
                "stats": {"requests": 0, "blocked": 0, "captchas": 0},
                "start_time": START_TIME * 1000
            }
            
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(json.dumps(response).encode())
            
        except Exception as e:
            self.send_response(500)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"error": str(e)}).encode())
    
    def handle_log_stream(self):
        """Stream logs via SSE."""
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        
        # Send initial message
        msg = {
            "time": time.strftime("%H:%M:%S"),
            "level": "info",
            "source": "dashboard",
            "message": "Connected to log stream"
        }
        self.wfile.write(f"data: {json.dumps(msg)}\n\n".encode())
        self.wfile.flush()
        
        # Keep connection alive with periodic updates
        try:
            while True:
                time.sleep(5)
                msg = {
                    "time": time.strftime("%H:%M:%S"),
                    "level": "debug",
                    "source": "dashboard", 
                    "message": "Heartbeat"
                }
                self.wfile.write(f"data: {json.dumps(msg)}\n\n".encode())
                self.wfile.flush()
        except:
            pass


class ThreadedHTTPServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
    allow_reuse_address = True
    daemon_threads = True


def main():
    print(f"\n🔱 Cerberus Dashboard Server")
    print(f"{'=' * 40}")
    print(f"Dashboard: http://127.0.0.1:{PORT}/")
    print(f"API:       http://127.0.0.1:{PORT}/api/status")
    print(f"{'=' * 40}\n")
    
    # Verify dashboard files exist
    index_file = DASHBOARD_DIR / "index.html"
    if not index_file.exists():
        print(f"ERROR: Dashboard files not found at {DASHBOARD_DIR}")
        print(f"Expected: {index_file}")
        return
    
    print(f"Serving from: {DASHBOARD_DIR}")
    
    try:
        with ThreadedHTTPServer(("", PORT), DashboardHandler) as httpd:
            print(f"Server started on port {PORT}")
            httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down...")
    except OSError as e:
        print(f"ERROR: {e}")
        if "Address already in use" in str(e):
            print(f"Port {PORT} is in use. Kill it with: sudo kill $(sudo lsof -t -i:{PORT})")


if __name__ == "__main__":
    main()
