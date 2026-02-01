/*
 * Cerberus XDP Kernel Program
 * L2 Defense: Volumetric Flood Protection
 *
 * Drops SYN floods and rate-limits per-IP traffic at the driver level,
 * before the kernel allocates any memory for the connection.
 *
 * Build: make
 * Load: ./scripts/cerberus-init.sh
 */

#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ip.h>
#include <linux/udp.h>
#include <linux/tcp.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

/* --- Configuration Constants --- */
#define MAX_PPS_PER_IP 5000            /* Packets Per Second Allowed */
#define BLOCK_DURATION 60000000000ULL  /* 60 seconds in nanoseconds */
#define WIREGUARD_PORT 51820           /* WireGuard default port */

/* --- Data Structures --- */

struct rate_info {
    __u64 last_seen;
    __u64 packet_count;
};

/* --- BPF Maps --- */

/*
 * Rate Limit Map (LRU Hash)
 * Key: Source IP (u32)
 * Value: struct rate_info
 * 
 * Tracks per-IP packet counts with automatic LRU eviction.
 */
struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 100000);  /* Track up to 100k distinct IPs */
    __type(key, __u32);
    __type(value, struct rate_info);
} rate_map SEC(".maps");

/*
 * Block List (LRU Hash)
 * Key: Source IP (u32)
 * Value: Expiry Timestamp (u64)
 *
 * IPs that exceeded rate limits are blocked until expiry.
 */
struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 10000);
    __type(key, __u32);
    __type(value, __u64);
} block_map SEC(".maps");

/*
 * Metrics Counters (Per-CPU Array)
 * Index 0: Packets passed
 * Index 1: Packets dropped (rate limit)
 * Index 2: Packets dropped (blocked)
 */
struct {
    __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
    __uint(max_entries, 8);
    __type(key, __u32);
    __type(value, __u64);
} metrics SEC(".maps");

/* --- Helper Functions --- */

static __always_inline void increment_metric(__u32 index) {
    __u64 *counter = bpf_map_lookup_elem(&metrics, &index);
    if (counter) {
        (*counter)++;
    }
}

/*
 * Check and update rate limit for a source IP.
 * Returns XDP_PASS if within limits, XDP_DROP if exceeded.
 */
static __always_inline int check_rate_limit(__u32 src_ip) {
    __u64 now = bpf_ktime_get_ns();
    struct rate_info *info = bpf_map_lookup_elem(&rate_map, &src_ip);
    
    if (!info) {
        /* New IP: Initialize tracking */
        struct rate_info new_info = { .last_seen = now, .packet_count = 1 };
        bpf_map_update_elem(&rate_map, &src_ip, &new_info, BPF_ANY);
        return XDP_PASS;
    }

    /* Reset counter every second */
    if (now - info->last_seen > 1000000000ULL) {
        info->last_seen = now;
        info->packet_count = 1;
    } else {
        info->packet_count++;
        
        if (info->packet_count > MAX_PPS_PER_IP) {
            /* Threshold exceeded: Add to block map */
            __u64 expiry = now + BLOCK_DURATION;
            bpf_map_update_elem(&block_map, &src_ip, &expiry, BPF_ANY);
            increment_metric(1);  /* Metric: dropped by rate limit */
            return XDP_DROP;
        }
    }
    
    return XDP_PASS;
}

/* --- Main XDP Program --- */

SEC("xdp")
int cerberus_firewall(struct xdp_md *ctx) {
    void *data_end = (void *)(long)ctx->data_end;
    void *data = (void *)(long)ctx->data;
    struct ethhdr *eth = data;

    /* Sanity Check: Valid Ethernet header */
    if ((void *)(eth + 1) > data_end)
        return XDP_PASS;

    /* Only process IPv4 */
    if (eth->h_proto != bpf_htons(ETH_P_IP))
        return XDP_PASS;

    struct iphdr *ip = (void *)(eth + 1);
    if ((void *)(ip + 1) > data_end)
        return XDP_PASS;
    
    __u32 src_ip = ip->saddr;
    __u64 now = bpf_ktime_get_ns();

    /* Fast Path: Check Block List First */
    __u64 *expiry = bpf_map_lookup_elem(&block_map, &src_ip);
    if (expiry) {
        if (now < *expiry) {
            increment_metric(2);  /* Metric: dropped by block */
            return XDP_DROP;      /* Still blocked */
        }
        /* Expired: Remove from block list */
        bpf_map_delete_elem(&block_map, &src_ip);
    }

    /* Protocol-Specific Handling */
    
    /* TCP: Rate limit all TCP traffic */
    if (ip->protocol == IPPROTO_TCP) {
        struct tcphdr *tcp = (void *)(ip + 1);
        if ((void *)(tcp + 1) > data_end)
            return XDP_DROP;  /* Malformed TCP */
        
        return check_rate_limit(src_ip);
    }
    
    /* UDP: Allow only WireGuard (port 51820) */
    if (ip->protocol == IPPROTO_UDP) {
        struct udphdr *udp = (void *)(ip + 1);
        if ((void *)(udp + 1) > data_end)
            return XDP_PASS;  /* Malformed UDP -> let kernel validate */
        
        if (udp->dest != bpf_htons(WIREGUARD_PORT)) {
            /* Non-WireGuard UDP: Pass to kernel for DNS/DHCP etc. */
            return XDP_PASS;
        }
        
        /* WireGuard: Rate limit like TCP */
        return check_rate_limit(src_ip);
    }

    /* ICMP, other protocols: Pass through */
    increment_metric(0);  /* Metric: passed */
    
    /*
     * Default Action: PASS
     * 
     * MVP Safety: Allow unknown traffic through.
     * TODO: Harden to XDP_DROP once allowlist is exhaustive
     *       (SSH, ICMP echo, DNS, DHCP, etc.)
     */
    return XDP_PASS;
}

char _license[] SEC("license") = "GPL";
