#ifndef VELOS_CORE_H
#define VELOS_CORE_H

#include <stdbool.h>
#include <stdint.h>

// ============================================================
// Types
// ============================================================

typedef struct {
    const char* name;
    const char* script;
    const char* cwd;
    const char* interpreter;  // NULL = auto-detect
    uint32_t kill_timeout_ms; // default: 5000
    bool autorestart;
    int32_t max_restarts;     // -1 = unlimited, default: 15
    uint64_t min_uptime_ms;   // default: 1000
    uint32_t restart_delay_ms;// default: 0
    bool exp_backoff;
} VelosProcessConfig;

typedef struct {
    uint32_t id;
    const char* name;
    uint32_t pid;
    uint8_t status;          // 0=stopped, 1=running, 2=errored, 3=starting
    uint64_t memory_bytes;
    uint64_t uptime_ms;
    uint32_t restart_count;
} VelosProcessInfo;

typedef struct {
    uint64_t timestamp_ms;
    uint8_t level;           // 0=debug, 1=info, 2=warn, 3=error
    uint8_t stream;          // 0=stdout, 1=stderr
    const uint8_t* message;
    uint32_t message_len;
} VelosLogEntry;

// ============================================================
// Daemon lifecycle
// ============================================================

const char* velos_ping(void);
int velos_daemon_init(const char* socket_path, const char* state_dir);
int velos_daemon_run(void);
int velos_daemon_shutdown(void);

// ============================================================
// Process management
// ============================================================

int velos_process_start(const VelosProcessConfig* config);
int velos_process_stop(uint32_t process_id, int signal, uint32_t timeout_ms);
int velos_process_restart(uint32_t process_id);
int velos_process_delete(uint32_t process_id);
int velos_process_list(VelosProcessInfo** out, uint32_t* count);
void velos_process_list_free(VelosProcessInfo* list, uint32_t count);

// ============================================================
// Logging
// ============================================================

int velos_log_read(uint32_t process_id, uint32_t lines, VelosLogEntry** out, uint32_t* count);
void velos_log_free(VelosLogEntry* entries, uint32_t count);

// ============================================================
// State persistence
// ============================================================

int velos_state_save(void);
int velos_state_load(void);

#endif
