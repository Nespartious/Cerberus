//! HAProxy Runtime API Integration
//!
//! Communicates with HAProxy via its Unix socket Runtime API to:
//! - Update circuit status in stick tables (VIP/Ban)
//! - Query current connection statistics
//! - Read stick table entries
//!
//! Reference: https://www.haproxy.com/blog/dynamic-configuration-haproxy-runtime-api/
//!
//! NOTE: Unix sockets are only available on Unix systems. On Windows,
//! this module provides stub implementations that log warnings.

use anyhow::Result;

/// HAProxy Runtime API client
#[allow(dead_code)]
pub struct HaproxyApi {
    /// Path to HAProxy runtime socket
    socket_path: String,
    /// Stick table name for circuit tracking
    stick_table: String,
}

/// Circuit status values in HAProxy stick table gpc0
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum HaproxyCircuitStatus {
    /// Normal user (default)
    Normal = 0,
    /// VIP - bypasses rate limits
    Vip = 1,
    /// Banned - denied at HAProxy level
    Banned = 2,
}

#[allow(dead_code)]
impl HaproxyApi {
    /// Create a new HAProxy API client
    pub fn new(socket_path: String, stick_table: String) -> Self {
        Self {
            socket_path,
            stick_table,
        }
    }

    /// Create with default paths
    pub fn default_paths() -> Self {
        Self {
            socket_path: "/var/run/haproxy.sock".to_string(),
            stick_table: "be_stick_tables".to_string(),
        }
    }

    /// Check if socket is accessible
    pub async fn is_available(&self) -> bool {
        #[cfg(unix)]
        {
            std::path::Path::new(&self.socket_path).exists()
        }
        #[cfg(not(unix))]
        {
            false
        }
    }

    /// Execute a command and return the response (Unix only)
    #[cfg(unix)]
    async fn execute(&self, command: &str) -> Result<String> {
        use anyhow::Context;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to HAProxy socket")?;

        stream
            .write_all(format!("{}\n", command).as_bytes())
            .await
            .context("Failed to send command to HAProxy")?;

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        let mut line = String::new();

        while reader.read_line(&mut line).await? > 0 {
            response.push_str(&line);
            line.clear();
        }

        Ok(response.trim().to_string())
    }

    /// Execute a command (Windows stub - not supported)
    #[cfg(not(unix))]
    async fn execute(&self, _command: &str) -> Result<String> {
        tracing::debug!("HAProxy socket not available on Windows");
        Ok(String::new())
    }

    /// Set circuit status in stick table
    pub async fn set_circuit_status(
        &self,
        circuit_id: &str,
        status: HaproxyCircuitStatus,
    ) -> Result<()> {
        if !self.is_available().await {
            tracing::debug!("HAProxy socket not available, skipping stick table update");
            return Ok(());
        }

        let command = format!(
            "set table {} key {} data.gpc0 {}",
            self.stick_table, circuit_id, status as u8
        );

        let response = self.execute(&command).await?;

        if !response.is_empty() && !response.starts_with("Entry") {
            tracing::warn!(
                circuit_id = circuit_id,
                response = response,
                "Unexpected HAProxy response"
            );
        }

        tracing::debug!(
            circuit_id = circuit_id,
            status = ?status,
            "Updated HAProxy stick table"
        );

        Ok(())
    }

    /// Promote a circuit to VIP status
    pub async fn promote_to_vip(&self, circuit_id: &str) -> Result<()> {
        self.set_circuit_status(circuit_id, HaproxyCircuitStatus::Vip)
            .await
    }

    /// Ban a circuit at HAProxy level
    pub async fn ban_circuit(&self, circuit_id: &str) -> Result<()> {
        self.set_circuit_status(circuit_id, HaproxyCircuitStatus::Banned)
            .await
    }

    /// Remove a circuit from stick table
    pub async fn clear_circuit(&self, circuit_id: &str) -> Result<()> {
        if !self.is_available().await {
            return Ok(());
        }

        let command = format!("clear table {} key {}", self.stick_table, circuit_id);
        let _ = self.execute(&command).await?;

        tracing::debug!(circuit_id = circuit_id, "Cleared HAProxy stick table entry");

        Ok(())
    }

    /// Get circuit info from stick table
    pub async fn get_circuit_info(&self, circuit_id: &str) -> Result<Option<StickTableEntry>> {
        if !self.is_available().await {
            return Ok(None);
        }

        let command = format!("show table {} key {}", self.stick_table, circuit_id);
        let response = self.execute(&command).await?;

        if response.is_empty() || response.contains("not found") {
            return Ok(None);
        }

        for line in response.lines() {
            if line.contains(&format!("key={}", circuit_id)) || line.contains(circuit_id) {
                return Ok(Some(StickTableEntry::parse(line)?));
            }
        }

        Ok(None)
    }

    /// Get HAProxy statistics
    pub async fn get_stats(&self) -> Result<HaproxyStats> {
        if !self.is_available().await {
            return Ok(HaproxyStats::default());
        }

        let response = self.execute("show stat").await?;
        let mut stats = HaproxyStats::default();

        for line in response.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() < 10 {
                continue;
            }

            if let Ok(scur) = fields.get(4).unwrap_or(&"0").parse::<u64>() {
                stats.current_sessions += scur;
            }

            if let Ok(stot) = fields.get(7).unwrap_or(&"0").parse::<u64>() {
                stats.total_sessions += stot;
            }
        }

        Ok(stats)
    }

    /// Get stick table statistics
    pub async fn get_table_stats(&self) -> Result<TableStats> {
        if !self.is_available().await {
            return Ok(TableStats::default());
        }

        let command = format!("show table {}", self.stick_table);
        let response = self.execute(&command).await?;
        let mut stats = TableStats::default();

        if let Some(header) = response.lines().next() {
            if let Some(used_part) = header.split("used:").nth(1) {
                if let Ok(used) = used_part.trim().parse::<u64>() {
                    stats.entries_used = used;
                }
            }
            if let Some(size_part) = header.split("size:").nth(1) {
                if let Some(size_str) = size_part.split(',').next() {
                    if let Ok(size) = size_str.trim().parse::<u64>() {
                        stats.entries_max = size;
                    }
                }
            }
        }

        Ok(stats)
    }
}

/// Parsed stick table entry
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct StickTableEntry {
    pub conn_cur: u32,
    pub conn_rate: u32,
    pub http_req_rate: u32,
    pub gpc0: u8,
    pub expire_secs: u64,
}

impl StickTableEntry {
    fn parse(line: &str) -> Result<Self> {
        let mut entry = StickTableEntry::default();

        for part in line.split_whitespace() {
            if let Some(val) = part.strip_prefix("conn_cur=") {
                entry.conn_cur = val.parse().unwrap_or(0);
            } else if part.starts_with("conn_rate") {
                if let Some(eq_pos) = part.find('=') {
                    entry.conn_rate = part[eq_pos + 1..].parse().unwrap_or(0);
                }
            } else if part.starts_with("http_req_rate") {
                if let Some(eq_pos) = part.find('=') {
                    entry.http_req_rate = part[eq_pos + 1..].parse().unwrap_or(0);
                }
            } else if let Some(val) = part.strip_prefix("gpc0=") {
                entry.gpc0 = val.parse().unwrap_or(0);
            } else if let Some(val) = part.strip_prefix("exp=") {
                entry.expire_secs = val.parse().unwrap_or(0);
            }
        }

        Ok(entry)
    }
}

/// HAProxy runtime statistics
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct HaproxyStats {
    pub current_sessions: u64,
    pub total_sessions: u64,
}

/// Stick table statistics
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct TableStats {
    pub entries_used: u64,
    pub entries_max: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stick_table_entry_parse() {
        let line = "0x12345678: key=abc123 use=1 exp=1800 conn_cur=3 conn_rate(10000)=5 http_req_rate(10000)=10 gpc0=1";
        let entry = StickTableEntry::parse(line).unwrap();

        assert_eq!(entry.conn_cur, 3);
        assert_eq!(entry.conn_rate, 5);
        assert_eq!(entry.http_req_rate, 10);
        assert_eq!(entry.gpc0, 1);
        assert_eq!(entry.expire_secs, 1800);
    }
}
