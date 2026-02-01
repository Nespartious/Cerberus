/*
 * Cerberus TC eBPF Kernel Program
 * L3 Defense: Traffic Control Fallback
 *
 * Used when XDP Native/Generic modes are unavailable.
 * Provides similar rate limiting but at the Traffic Control layer.
 *
 * Build: make
 * Load: tc filter add dev eth0 ingress bpf da obj cerberus_tc.o sec ingress_firewall
 */

#include <linux/bpf.h>
#include <linux/pkt_cls.h>
#include <linux/if_ether.h>
#include <linux/ip.h>
#include <linux/tcp.h>
#include <linux/udp.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

/* --- Configuration --- */
#define MAX_PPS_PER_IP 5000
#define BLOCK_DURATION 60000000000ULL
#define WIREGUARD_PORT 51820

/* --- Data Structures --- */

struct rate_info {
    __u64 last_seen;
    __u64 packet_count;
};

/* --- BPF Maps (shared with XDP if loaded together) --- */

struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 100000);
    __type(key, __u32);
    __type(value, struct rate_info);
} tc_rate_map SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 10000);
    __type(key, __u32);
    __type(value, __u64);
} tc_block_map SEC(".maps");

/* --- Helper Functions --- */

static __always_inline int tc_check_rate_limit(__u32 src_ip) {
    __u64 now = bpf_ktime_get_ns();
    struct rate_info *info = bpf_map_lookup_elem(&tc_rate_map, &src_ip);
    
    if (!info) {
        struct rate_info new_info = { .last_seen = now, .packet_count = 1 };
        bpf_map_update_elem(&tc_rate_map, &src_ip, &new_info, BPF_ANY);
        return TC_ACT_OK;
    }

    if (now - info->last_seen > 1000000000ULL) {
        info->last_seen = now;
        info->packet_count = 1;
    } else {
        info->packet_count++;
        
        if (info->packet_count > MAX_PPS_PER_IP) {
            __u64 expiry = now + BLOCK_DURATION;
            bpf_map_update_elem(&tc_block_map, &src_ip, &expiry, BPF_ANY);
            return TC_ACT_SHOT;  /* Drop packet */
        }
    }
    
    return TC_ACT_OK;
}

/* --- Main TC Program --- */

SEC("ingress_firewall")
int cerberus_tc_ingress(struct __sk_buff *skb) {
    void *data_end = (void *)(long)skb->data_end;
    void *data = (void *)(long)skb->data;
    struct ethhdr *eth = data;

    if ((void *)(eth + 1) > data_end)
        return TC_ACT_OK;

    if (eth->h_proto != bpf_htons(ETH_P_IP))
        return TC_ACT_OK;

    struct iphdr *ip = (void *)(eth + 1);
    if ((void *)(ip + 1) > data_end)
        return TC_ACT_OK;
    
    __u32 src_ip = ip->saddr;
    __u64 now = bpf_ktime_get_ns();

    /* Check Block List */
    __u64 *expiry = bpf_map_lookup_elem(&tc_block_map, &src_ip);
    if (expiry) {
        if (now < *expiry) {
            return TC_ACT_SHOT;
        }
        bpf_map_delete_elem(&tc_block_map, &src_ip);
    }

    /* TCP: Rate limit */
    if (ip->protocol == IPPROTO_TCP) {
        struct tcphdr *tcp = (void *)(ip + 1);
        if ((void *)(tcp + 1) > data_end)
            return TC_ACT_SHOT;
        
        return tc_check_rate_limit(src_ip);
    }
    
    /* UDP: Allow WireGuard, rate limit */
    if (ip->protocol == IPPROTO_UDP) {
        struct udphdr *udp = (void *)(ip + 1);
        if ((void *)(udp + 1) > data_end)
            return TC_ACT_OK;
        
        if (udp->dest == bpf_htons(WIREGUARD_PORT)) {
            return tc_check_rate_limit(src_ip);
        }
    }

    return TC_ACT_OK;
}

char _license[] SEC("license") = "GPL";
