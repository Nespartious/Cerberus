# The Deployment of XDP/eBPF on VPS Infrastructure

The deployment of XDP/eBPF-based systems like Cerberus on standard Virtual Private Servers (VPS) presents a significant technical barrier due to how virtualization interacts with the Linux kernel and network drivers. While traditional software like Endgame V3 (Nginx/Lua) operates at the application layer and is broadly compatible, Cerberus's "Layer 0" protection requires deep integration with hardware that most cloud providers restrict for security or performance reasons.

The following sections detail the specific challenges regarding kernel versioning, execution modes, and virtualization drivers.

## 1. The Kernel 5.10 Threshold: Chaining and Dispatchers
The requirement for Linux Kernel $\ge$ 5.10 is primarily driven by the evolution of eBPF modularity.

* **Multi-Program Dispatchers:** Prior to kernel 5.10, attaching multiple eBPF programs to a single network interface was difficult and often required manual "tail calls" (where one program explicitly points to the next). Kernel 5.10 introduced a formal multi-program dispatcher that allows the kernel to manage a chain of independent XDP programs. For a modular system like Cerberus, this is essential to allow different defensive modules to run in sequence without recompiling a monolithic block of code.
* **BTF and CO-RE (Compile Once – Run Everywhere):** Modern eBPF relies on BPF Type Format (BTF) to understand kernel data structures at runtime. This allows a single binary to run on various kernel versions. While BTF began appearing earlier, it became standard in most distributions starting with 5.10. Many older VPS kernels are compiled without `CONFIG_DEBUG_INFO_BTF=y`, making it impossible to load advanced eBPF sensors.

## 2. The Performance Trap: Native vs. Generic Mode
XDP can execute in three "modes," and the choice determines whether the system actually provides DDoS protection or merely adds overhead.

* **Native (Driver) Mode:** The program runs directly in the network card (NIC) driver before any memory is allocated for the packet in the Linux networking stack. This is how XDP achieves "line-rate" packet dropping.
* **Generic (SKB) Mode:** If the driver does not support XDP (common in many virtualized environments), the kernel falls back to Generic mode. Here, the packet has already been processed by the kernel's stack and allocated a socket buffer (sk_buff).

**The Challenge:** Most standard VPS providers use paravirtualized drivers that force XDP into Generic mode. In this state, a DDoS attack can still saturate the kernel's memory and CPU because the "dropping" happens too late in the pipeline. To get the benefits Cerberus promises, the VPS host must support Native mode for the virtio-net driver, which many budget or standard providers do not enable.

## 3. Driver Hurdles: virtio-net, Multiqueue, and LRO
Even when a VPS runs a modern kernel, the virtualized network driver (virtio-net) requires specific, non-default configurations to support high-performance XDP:

* **Multiqueue Support:** To run XDP in native mode on a VM, the network interface must have multiqueue enabled. Specifically, it often requires a number of queues at least double the number of CPU cores. Many standard VPS plans (like those with only 1 or 2 vCPUs) are restricted to a single queue by the provider.
* **LRO (Large Receive Offload):** XDP native mode is technically incompatible with LRO. To load an XDP program, LRO must be disabled. However, many cloud providers implement LRO at the hypervisor level to improve performance, and attempting to disable it from within the guest VM often results in an "Operation not supported" error.

## 4. Cloud Provider Specific Limitations
Different cloud ecosystems present unique "deployment traps" for eBPF:

* **AWS EC2:** While AWS supports XDP on its `ena` (Elastic Network Adapter) driver, it suffers from a small TX ring size (1024). This creates a bottleneck for XDP programs that need to redirect or reflect traffic (like a Load Balancer or a behavioral trap), making them perform worse in "native" mode than they would in "generic" mode on the same hardware.
* **Shared vs. Dedicated Resources:** Standard VPS providers often use "shared" vCPUs. Since eBPF programs run in the context of the CPU handling the network interrupt, noisy neighbors on a shared host can introduce jitter that breaks the sub-microsecond latency requirements of a high-speed firewall.
* **Stripped-Down Kernels:** Many "Optimized" or "Cloud-Native" OS images provided by VPS vendors strip out non-essential kernel features to reduce boot time. This often includes removing the very eBPF helper functions and maps required by the Cerberus logic engine.
