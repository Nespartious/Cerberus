# 0205 - Attack Kill Table: What Dies at Which Layer

| Attack Type         | XDP | TC eBPF | Kernel TCP | HAProxy | Nginx | Fortify |
|---------------------|-----|---------|------------|---------|-------|---------|
| Packet flood        | ✅  | —       | —          | —       | —     | —       |
| SYN flood           | ✅  | ✅      | ✅         | —       | —     | —       |
| TCP exhaustion      | —   | ✅      | ✅         | ✅      | —     | —       |
| Connection churn    | —   | ✅      | —          | ✅      | —     | —       |
| Slowloris           | —   | —       | —          | ✅      | ✅    | —       |
| HTTP floods         | —   | —       | —          | ✅      | ✅    | —       |
| Malformed HTTP      | —   | —       | —          | —       | ✅    | —       |
| CAPTCHA bypass      | —   | —       | —          | —       | —     | ✅      |
| Bot navigation      | —   | —       | —          | —       | —     | ✅      |
| CAPTCHA farms       | —   | —       | —          | —       | —     | ⚠️      |
| Human users         | ❌  | ❌      | ❌         | ❌      | ❌    | ❌      |

Legend:
- ✅ = attack dies here
- ⚠️ = attack slowed, not killed
- ❌ = allowed
- — = not relevant

---

> This table is the basis for Cerberus’ multi-layer defense design.
