# Cerberus XDP/eBPF Build System
#
# Compiles the eBPF kernel program for L2 defense.
# Requires: clang, llc, libbpf-dev

CLANG ?= clang
LLC ?= llc
ARCH := $(shell uname -m | sed 's/x86_64/x86/' | sed 's/aarch64/arm64/')

BPF_CFLAGS ?= -O2 -g -target bpf -D__TARGET_ARCH_$(ARCH)

# Include paths for BPF headers
INCLUDES := -I/usr/include/bpf -I/usr/include

# Output directory
BUILD_DIR := build

.PHONY: all clean install

all: $(BUILD_DIR)/cerberus_xdp.o $(BUILD_DIR)/cerberus_tc.o

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# XDP Program (L2 - Driver Level)
$(BUILD_DIR)/cerberus_xdp.o: cerberus_xdp_kern.c | $(BUILD_DIR)
	$(CLANG) $(BPF_CFLAGS) $(INCLUDES) -c $< -o $@

# TC Program (L3 - Traffic Control Fallback)
$(BUILD_DIR)/cerberus_tc.o: cerberus_tc_kern.c | $(BUILD_DIR)
	$(CLANG) $(BPF_CFLAGS) $(INCLUDES) -c $< -o $@

clean:
	rm -rf $(BUILD_DIR)

install: all
	@echo "Installing XDP programs to /opt/cerberus/xdp/"
	mkdir -p /opt/cerberus/xdp
	cp $(BUILD_DIR)/*.o /opt/cerberus/xdp/
	@echo "✅ XDP programs installed"

# Verify the BPF programs
verify: all
	@echo "Verifying BPF programs with bpftool..."
	bpftool prog load $(BUILD_DIR)/cerberus_xdp.o /sys/fs/bpf/cerberus_test 2>/dev/null && \
		rm /sys/fs/bpf/cerberus_test && \
		echo "✅ cerberus_xdp.o verified" || \
		echo "❌ cerberus_xdp.o failed verification"
