// Disposable CI mechanism fixture only. No product IPC, certified bundle or owner authorization.
#include <Security/Security.h>
#include <bsm/libbsm.h>
#include <errno.h>
#include <mach/mach.h>
#include <servers/bootstrap.h>
#include <signal.h>
#include <spawn.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

extern char **environ;
#define FIXTURE_ID "com.cryptex.m8.bootstrap.mechanism"
#define MESSAGE_ID 0x43584232
#define WAIT_MS 2500
enum { OK=0, BAD_LAYOUT=50, WRONG_SESSION, WRONG_PID, WRONG_GENERATION, NOT_LIVE,
       BAD_CODE, REPLAY, PARENT_CHANGED, LOOKUP_FAILED, SEND_FAILED, RECEIVE_FAILED,
       CHILD_DEADLINE, SETUP_FAILED, SUPERVISOR_DEADLINE, CHILD_SIGNAL, WAIT_FAILED,
       NAMESPACE_PRESENT, SEND_TIMEOUT, CLEANUP_REFUSED };
enum { HELLO=1, ACK, CONFIRM, STOP, REJECT };
typedef struct {
    mach_msg_header_t header;
    mach_msg_body_t body; // Only zero-descriptor simple messages are accepted.
    uint32_t version, operation;
    unsigned char session[16]; // Nonsecret freshness data, never identity.
    uint64_t sequence;
    int32_t claimed_pid; // Deliberately ignored untrusted data.
} Message;
typedef union { uint64_t alignment; unsigned char bytes[2048]; } Buffer;
typedef struct { Message message; audit_token_t sender; mach_port_t reply; } Frame;
typedef struct {
    pid_t pid;
    audit_token_t token;
    mach_port_t task;
    uint64_t sequence;
} Pin;
typedef struct { const char *stage; int code; } Exit;
static SecRequirementRef fixture_requirement;
static CFDataRef fixture_hash;
static int failures, spawn_error, cleanup_error;

static long long now_ms(void) {
    struct timespec value;
    if (clock_gettime(CLOCK_MONOTONIC, &value)) _exit(SETUP_FAILED);
    return (long long)value.tv_sec * 1000 + value.tv_nsec / 1000000;
}
static void drop_send(mach_port_t *right) {
    if (MACH_PORT_VALID(*right)) {
        kern_return_t result = mach_port_deallocate(mach_task_self(), *right);
        if (result != KERN_SUCCESS && !cleanup_error) cleanup_error = result;
    }
    *right = MACH_PORT_NULL;
}
static bool drop_receive(mach_port_t *right) {
    kern_return_t result = MACH_PORT_VALID(*right) ?
        mach_port_mod_refs(mach_task_self(), *right, MACH_PORT_RIGHT_RECEIVE, -1) : KERN_SUCCESS;
    bool ok = result == KERN_SUCCESS;
    if (!ok && !cleanup_error) cleanup_error = result;
    *right = MACH_PORT_NULL;
    return ok;
}
static void pin_close(Pin *pin) { drop_send(&pin->task); memset(pin, 0, sizeof(*pin)); }
static void frame_close(Frame *frame) { drop_send(&frame->reply); memset(frame, 0, sizeof(*frame)); }
static void emit(const char *name, const char *observed, const char *stage, int code, bool pass) {
    printf("{\"case\":\"%s\",\"observed\":\"%s\",\"stage\":\"%s\",\"code\":%d,\"pass\":%s}\n",
           name, observed, stage, code, pass ? "true" : "false");
    fflush(stdout);
    if (!pass) ++failures;
}
static CFDataRef hash_of(SecCodeRef code) {
    CFDictionaryRef info = NULL;
    if (SecCodeCopySigningInformation((SecStaticCodeRef)code, 0, &info) != errSecSuccess || !info) return NULL;
    CFTypeRef hash = CFDictionaryGetValue(info, kSecCodeInfoUnique);
    CFDataRef result = hash && CFGetTypeID(hash) == CFDataGetTypeID() && CFDataGetLength(hash) > 0 ? CFRetain(hash) : NULL;
    CFRelease(info);
    return result;
}
static bool fixture_open(void) {
    // Explicit ad-hoc CI identity. This is NOT the signed product policy.
    SecCodeRef self = NULL;
    if (SecRequirementCreateWithString(CFSTR("identifier \"" FIXTURE_ID "\""), 0, &fixture_requirement) != errSecSuccess ||
        SecCodeCopySelf(0, &self) != errSecSuccess || !self) return false;
    bool valid = SecCodeCheckValidity(self, kSecCSStrictValidate, fixture_requirement) == errSecSuccess;
    if (valid) fixture_hash = hash_of(self);
    CFRelease(self);
    return valid && fixture_hash;
}
static bool code_valid(const audit_token_t *token, pid_t expected) {
    CFDataRef audit = CFDataCreate(NULL, (const UInt8 *)token, sizeof(*token));
    CFNumberRef pid = CFNumberCreate(NULL, kCFNumberIntType, &expected);
    CFDictionaryRef attrs = NULL;
    SecCodeRef code = NULL;
    CFDataRef hash = NULL;
    bool valid = false;
    if (!audit || !pid) goto done;
    const void *keys[] = {kSecGuestAttributeAudit, kSecGuestAttributePid};
    const void *values[] = {audit, pid};
    attrs = CFDictionaryCreate(NULL, keys, values, 2, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    if (!attrs || SecCodeCopyGuestWithAttributes(NULL, attrs, 0, &code) != errSecSuccess || !code ||
        SecCodeCheckValidity(code, kSecCSStrictValidate, fixture_requirement) != errSecSuccess) goto done;
    hash = hash_of(code);
    valid = hash && CFEqual(hash, fixture_hash);
done:
    if (hash) CFRelease(hash);
    if (code) CFRelease(code);
    if (attrs) CFRelease(attrs);
    if (pid) CFRelease(pid);
    if (audit) CFRelease(audit);
    return valid;
}
static bool task_live(const Pin *pin) {
    mach_port_type_t type = 0;
    pid_t pid = 0;
    return MACH_PORT_VALID(pin->task) &&
        mach_port_type(mach_task_self(), pin->task, &type) == KERN_SUCCESS &&
        (type & MACH_PORT_TYPE_SEND) && !(type & MACH_PORT_TYPE_DEAD_NAME) &&
        pid_for_task(pin->task, &pid) == KERN_SUCCESS && pid == pin->pid;
}
static int pin_open(Pin *pin, const audit_token_t *token, pid_t expected) {
    memset(pin, 0, sizeof(*pin));
    if (expected <= 1 || audit_token_to_pid(*token) != expected) return WRONG_PID;
    if (!code_valid(token, expected)) return BAD_CODE;
    pin->pid = expected;
    pin->token = *token;
    if (task_name_for_pid(mach_task_self(), expected, &pin->task) != KERN_SUCCESS ||
        !task_live(pin) || !code_valid(token, expected)) { pin_close(pin); return NOT_LIVE; }
    return OK;
}
static int pin_check(const Pin *pin, const audit_token_t *token) {
    if (audit_token_to_pid(*token) != pin->pid) return WRONG_PID;
    if (audit_token_to_pidversion(*token) != audit_token_to_pidversion(pin->token)) return WRONG_GENERATION;
    if (!task_live(pin)) return NOT_LIVE;
    if (!code_valid(&pin->token, pin->pid) || !code_valid(token, pin->pid)) return BAD_CODE;
    return task_live(pin) ? OK : NOT_LIVE;
}
static pid_t spawn_fixed(const char *binary, char *const args[]) {
    posix_spawn_file_actions_t actions;
    posix_spawnattr_t attributes;
    spawn_error = posix_spawn_file_actions_init(&actions);
    if (spawn_error) return -1;
    spawn_error = posix_spawnattr_init(&attributes);
    if (spawn_error) { posix_spawn_file_actions_destroy(&actions); return -1; }
    spawn_error = posix_spawnattr_setflags(&attributes, POSIX_SPAWN_CLOEXEC_DEFAULT);
    for (int fd=0; !spawn_error && fd<3; ++fd) spawn_error = posix_spawn_file_actions_adddup2(&actions, fd, fd);
    pid_t child = -1;
    if (!spawn_error) spawn_error = posix_spawn(&child, binary, &actions, &attributes, args, environ);
    posix_spawn_file_actions_destroy(&actions);
    posix_spawnattr_destroy(&attributes);
    return spawn_error ? -1 : child;
}
typedef pid_t (*WaitFunction)(pid_t, int *, int);
typedef int (*KillFunction)(pid_t, int);
static Exit terminal_status(int status) {
    if (WIFEXITED(status)) return (Exit){"child_exit", WEXITSTATUS(status)};
    if (WIFSIGNALED(status)) return (Exit){"child_signal", WTERMSIG(status)};
    return (Exit){"wait", WAIT_FAILED};
}
static Exit reap_with(pid_t child, int timeout, WaitFunction wait_child, KillFunction kill_child) {
    long long deadline = now_ms() + timeout;
    int status = 0;
    while (now_ms() < deadline) {
        pid_t observed = wait_child(child, &status, WNOHANG);
        if (observed == child) return terminal_status(status);
        if (observed < 0 && errno != EINTR) return (Exit){"wait", errno};
        struct timespec pause = {.tv_sec=0, .tv_nsec=10000000}; nanosleep(&pause, NULL);
    }
    // Child has not been reaped; its PID remains reserved until kill + wait.
    int killed = kill_child(child, SIGKILL);
    if (killed < 0 && errno != ESRCH) return (Exit){"kill", errno};
    pid_t observed;
    do { observed = wait_child(child, &status, 0); } while (observed < 0 && errno == EINTR);
    if (observed != child) return (Exit){"wait", observed < 0 ? errno : WAIT_FAILED};
    Exit exit = terminal_status(status);
    if (killed == 0 && !strcmp(exit.stage, "child_signal") && exit.code == SIGKILL)
        return (Exit){"supervisor", SUPERVISOR_DEADLINE};
    return exit; // A raced normal exit/other signal is not our supervisor kill.
}
static Exit reap(pid_t child, int timeout) { return reap_with(child, timeout, waitpid, kill); }
static int send_message(mach_port_t remote, mach_port_t reply, bool once, uint32_t operation,
                        uint64_t sequence, const unsigned char session[16], int32_t claimed, int layout) {
    Message message = {0};
    message.header.msgh_bits = MACH_MSGH_BITS(once ? MACH_MSG_TYPE_MOVE_SEND_ONCE : MACH_MSG_TYPE_COPY_SEND,
                                            MACH_PORT_VALID(reply) ? MACH_MSG_TYPE_MAKE_SEND_ONCE : 0);
    message.header.msgh_size = layout == 1 ? sizeof(mach_msg_header_t) + sizeof(uint32_t) : sizeof(message);
    message.header.msgh_remote_port = remote;
    message.header.msgh_local_port = reply;
    message.header.msgh_id = MESSAGE_ID;
    message.version = 1; message.operation = operation; message.sequence = sequence; message.claimed_pid = claimed;
    memcpy(message.session, session, sizeof(message.session));
    struct { mach_msg_header_t header; mach_msg_body_t body; mach_msg_port_descriptor_t right; } complex = {0};
    mach_msg_header_t *packet = &message.header;
    if (layout == 2) {
        complex.header = message.header;
        complex.header.msgh_bits |= MACH_MSGH_BITS_COMPLEX;
        complex.header.msgh_size = sizeof(complex);
        complex.body.msgh_descriptor_count = 1;
        complex.right.name = remote;
        complex.right.disposition = MACH_MSG_TYPE_COPY_SEND;
        complex.right.type = MACH_MSG_PORT_DESCRIPTOR;
        packet = &complex.header;
    }
    mach_msg_return_t result = mach_msg(packet, MACH_SEND_MSG | MACH_SEND_TIMEOUT,
        packet->msgh_size, 0, MACH_PORT_NULL, WAIT_MS, MACH_PORT_NULL);
    // A failed send may pseudo-receive rights under different names. Never
    // clean up borrowed/outgoing fields as if they were a received message.
    // This disposable fixture fails immediately; process exit drops its entire
    // IPC space. The Python supervisor separately stops its still-owned group.
    if (result != MACH_MSG_SUCCESS) _exit(result == MACH_SEND_TIMED_OUT ? SEND_TIMEOUT : SEND_FAILED);
    return OK;
}
static int receive_message(mach_port_t port, Frame *frame, int timeout) {
    memset(frame, 0, sizeof(*frame));
    Buffer buffer = {0};
    mach_msg_header_t *header = (mach_msg_header_t *)buffer.bytes;
    mach_msg_option_t options = MACH_RCV_MSG | MACH_RCV_TIMEOUT |
        MACH_RCV_TRAILER_TYPE(MACH_MSG_TRAILER_FORMAT_0) | MACH_RCV_TRAILER_ELEMENTS(MACH_RCV_TRAILER_AUDIT);
    mach_msg_return_t result = mach_msg(header, options, 0, sizeof(buffer), port, (mach_msg_timeout_t)timeout, MACH_PORT_NULL);
    if (result != MACH_MSG_SUCCESS) return result == MACH_RCV_TIMED_OUT ? CHILD_DEADLINE : RECEIVE_FAILED;
    int code = BAD_LAYOUT;
    if (header->msgh_bits & MACH_MSGH_BITS_COMPLEX || header->msgh_size != sizeof(Message) ||
        header->msgh_id != MESSAGE_ID || header->msgh_voucher_port != MACH_PORT_NULL ||
        (MACH_PORT_VALID(header->msgh_remote_port) && MACH_MSGH_BITS_REMOTE(header->msgh_bits) != MACH_MSG_TYPE_PORT_SEND_ONCE)) goto done;
    size_t offset = (header->msgh_size + 3u) & ~(size_t)3u;
    if (offset > sizeof(buffer) || sizeof(mach_msg_audit_trailer_t) > sizeof(buffer) - offset) goto done;
    mach_msg_audit_trailer_t trailer;
    memcpy(&trailer, buffer.bytes + offset, sizeof(trailer));
    if (trailer.msgh_trailer_type != MACH_MSG_TRAILER_FORMAT_0 || trailer.msgh_trailer_size != sizeof(trailer)) goto done;
    memcpy(&frame->message, buffer.bytes, sizeof(frame->message));
    if (frame->message.version != 1 || frame->message.body.msgh_descriptor_count != 0) goto done;
    frame->sender = trailer.msgh_audit;
    frame->reply = header->msgh_remote_port;
    header->msgh_remote_port = MACH_PORT_NULL; // Transfer the received send-once reference to Frame.
    code = OK;
done:
    mach_msg_destroy(header); // Also destroys rights carried by rejected complex frames.
    return code;
}
static int admit(Pin *pin, const Frame *frame, const unsigned char session[16], uint32_t operation) {
    int code = pin_check(pin, &frame->sender);
    if (code) return code;
    if (memcmp(frame->message.session, session, 16)) return WRONG_SESSION;
    if (!frame->message.sequence || frame->message.sequence <= pin->sequence) return REPLAY;
    if (frame->message.operation != operation) return BAD_LAYOUT;
    pin->sequence = frame->message.sequence;
    return OK;
}
static bool parse_session(const char *text, unsigned char bytes[16]) {
    if (strlen(text) != 32 || strspn(text, "0123456789abcdef") != 32) return false;
    for (size_t i=0; i<16; ++i) {
        char pair[3] = {text[2*i], text[2*i+1], 0}; bytes[i] = (unsigned char)strtoul(pair, NULL, 16);
    }
    return true;
}
static bool name_valid(const char *name) {
    const char *prefix = "com.cryptex.m8.bootstrap.";
    size_t length = strlen(prefix);
    if (strncmp(name, prefix, length) || strlen(name) < length + 26 || strlen(name) > length + 32) return false;
    for (size_t i=0; i<24; ++i) if (!strchr("0123456789abcdef", name[length+i])) return false;
    const char *suffix = name + length + 24;
    return !strcmp(suffix, ".forced") || (strlen(suffix) == 2 && suffix[0] == '.' && suffix[1] >= '0' && suffix[1] <= '8');
}
static int worker(const char *binary, const char *mode, const char *name, const char *text, int32_t claim) {
    unsigned char session[16];
    const char *modes[] = {"normal", "imposter", "replay", "wrong-session", "short", "complex", "exec", "postexec", "queued", "silent"};
    bool known = false;
    for (size_t i=0; i<sizeof(modes)/sizeof(modes[0]); ++i) known |= !strcmp(mode, modes[i]);
    if (!known || !name_valid(name) || !parse_session(text, session) || !fixture_open()) return BAD_CODE;
    pid_t parent = getppid();
    if (parent <= 1) return PARENT_CHANGED;
    mach_port_t service = MACH_PORT_NULL, reply = MACH_PORT_NULL;
    Pin pin = {0}; Frame frame = {0};
    int code = LOOKUP_FAILED;
    if (bootstrap_look_up(bootstrap_port, name, &service) != BOOTSTRAP_SUCCESS) goto done;
    if (!strcmp(mode, "silent")) { for (;;) pause(); }
    if (mach_port_allocate(mach_task_self(), MACH_PORT_RIGHT_RECEIVE, &reply) != KERN_SUCCESS) { code=SETUP_FAILED; goto done; }
    int layout = !strcmp(mode, "short") ? 1 : !strcmp(mode, "complex") ? 2 : 0;
    bool postexec = !strcmp(mode, "postexec");
    code = send_message(service, reply, false, postexec ? CONFIRM : HELLO, postexec ? 2 : 1, session, claim, layout);
    if (code || layout || !strcmp(mode, "imposter") || postexec) goto done;
    code = receive_message(reply, &frame, WAIT_MS);
    if (!code && getppid() != parent) code = PARENT_CHANGED;
    if (!code) code = pin_open(&pin, &frame.sender, parent);
    if (!code) code = admit(&pin, &frame, session, ACK);
    if (!code && MACH_PORT_VALID(frame.reply)) code = BAD_LAYOUT;
    frame_close(&frame);
    if (code) goto done;
    if (!strcmp(mode, "exec")) {
        execl(binary, binary, "worker", "postexec", name, text, "0", (char *)NULL);
        code = SETUP_FAILED; goto done;
    }
    if (!strcmp(mode, "wrong-session")) session[0] ^= 1;
    code = send_message(service, reply, false, CONFIRM, !strcmp(mode, "replay") ? 1 : 2, session, claim, 0);
    if (!strcmp(mode, "wrong-session")) session[0] ^= 1;
    if (code || !strcmp(mode, "queued")) goto done;
    code = receive_message(reply, &frame, WAIT_MS);
    if (!code && getppid() != parent) code = PARENT_CHANGED;
    if (!code) code = admit(&pin, &frame, session,
        !strcmp(mode, "normal") ? STOP : REJECT);
    if (!code && MACH_PORT_VALID(frame.reply)) code = BAD_LAYOUT;
done:
    frame_close(&frame); pin_close(&pin); drop_receive(&reply); drop_send(&service);
    if (fixture_hash) CFRelease(fixture_hash);
    if (fixture_requirement) CFRelease(fixture_requirement);
    return cleanup_error ? SETUP_FAILED : code;
}
static int lookup_missing(const char *name) {
    long long deadline = now_ms() + 3000;
    do {
        mach_port_t send = MACH_PORT_NULL;
        kern_return_t result = bootstrap_look_up(bootstrap_port, name, &send);
        drop_send(&send);
        if (result == BOOTSTRAP_UNKNOWN_SERVICE) return cleanup_error ? SETUP_FAILED : OK;
        if (result != BOOTSTRAP_SUCCESS) return LOOKUP_FAILED;
        struct timespec pause = {.tv_sec=0, .tv_nsec=20000000}; nanosleep(&pause, NULL);
    } while (now_ms() < deadline);
    return NAMESPACE_PRESENT;
}
static bool flow(const char *binary, const char *prefix, int index, const char *mode,
                 const char *test, int expected, bool intruder) {
    char name[BOOTSTRAP_MAX_NAME_LEN], text[33];
    unsigned char session[16]; arc4random_buf(session, sizeof(session));
    for (size_t i=0; i<16; ++i) snprintf(text+2*i, 3, "%02x", session[i]);
    snprintf(name, sizeof(name), "%s.%d", prefix, index);
    mach_port_t receive = MACH_PORT_NULL;
    kern_return_t registered = bootstrap_check_in(bootstrap_port, name, &receive);
    if (registered != BOOTSTRAP_SUCCESS) { emit(test, "bootstrap_error", "check_in", registered, false); return false; }
    if (intruder) {
        mach_port_t collision = MACH_PORT_NULL;
        kern_return_t result = bootstrap_check_in(bootstrap_port, name, &collision);
        bool refused = (result == BOOTSTRAP_SERVICE_ACTIVE || result == BOOTSTRAP_NAME_IN_USE) && !MACH_PORT_VALID(collision);
        emit("active_name_collision", refused ? "collision_refused" : "collision_not_refused", "check_in", result, refused);
        drop_receive(&collision);
    }
    char *args[] = {(char *)binary, "worker", (char *)mode, name, text, "0", NULL};
    pid_t child = spawn_fixed(binary, args);
    if (child < 0) { emit(test, "spawn_error", "spawn", spawn_error, false); drop_receive(&receive); return false; }
    Pin pin = {0}; Frame first = {0}, next = {0};
    int code = receive_message(receive, &first, !strcmp(mode, "silent") ? 100 : WAIT_MS);
    if (!code) code = pin_open(&pin, &first.sender, child);
    if (!code) code = admit(&pin, &first, session, HELLO);
    if (!code && !MACH_PORT_VALID(first.reply)) code = BAD_LAYOUT;
    if (!code && intruder) {
        char claimed[32]; snprintf(claimed, sizeof(claimed), "%d", child);
        char *other_args[] = {(char *)binary, "worker", "imposter", name, text, claimed, NULL};
        pid_t other = spawn_fixed(binary, other_args);
        if (other < 0) { emit("same_image_imposter", "spawn_error", "spawn", spawn_error, false); code=SETUP_FAILED; }
        else {
            int bad = receive_message(receive, &next, WAIT_MS);
            if (!bad) bad = admit(&pin, &next, session, HELLO);
            Exit other_exit = reap(other, WAIT_MS);
            bool pass = bad == WRONG_PID && !strcmp(other_exit.stage, "child_exit") && other_exit.code == 0 && pin.sequence == 1;
            emit("same_image_imposter", bad == WRONG_PID ? "sender_pid_refused" : "unexpected", "validate", bad, pass);
            frame_close(&next);
            if (!pass) code=SETUP_FAILED;
        }
    }
    if (!code) {
        mach_port_t target = first.reply; first.reply = MACH_PORT_NULL;
        code = send_message(target, MACH_PORT_NULL, true, ACK, 1, session, 0, 0);
    }
    Exit exit = {"not_waited", WAIT_FAILED};
    bool reaped = false;
    if (!code && !strcmp(mode, "queued")) { exit=reap(child, WAIT_MS); reaped=true; }
    if (!code) code = receive_message(receive, &next, WAIT_MS);
    if (!code) code = admit(&pin, &next, session, CONFIRM);
    bool expected_validation = code == expected;
    if ((code == OK || code == REPLAY || code == WRONG_SESSION) && MACH_PORT_VALID(next.reply)) {
        mach_port_t target = next.reply; next.reply = MACH_PORT_NULL;
        int sent = send_message(target, MACH_PORT_NULL, true, code == OK ? STOP : REJECT, 2, session, 0, 0);
        if (sent) { code=sent; expected_validation=false; }
    }
    if (!reaped) exit = reap(child, !strcmp(mode, "silent") ? 100 : WAIT_MS);
    bool child_ok = !strcmp(exit.stage, "child_exit") && exit.code == OK;
    bool pass = expected_validation && child_ok;
    const char *observed = code == OK ? "authenticated" : code == WRONG_PID ? "sender_pid_refused" :
        code == WRONG_GENERATION ? "sender_generation_refused" : code == NOT_LIVE ? "sender_not_live" :
        code == REPLAY ? "replay_refused" : code == WRONG_SESSION ? "session_refused" :
        code == BAD_LAYOUT ? "layout_refused" : code == CHILD_DEADLINE ? "receive_deadline" : "unexpected";
    if (!strcmp(mode, "silent")) {
        pass = code == CHILD_DEADLINE && !strcmp(exit.stage, "supervisor") && exit.code == SUPERVISOR_DEADLINE;
        emit(test, pass ? "supervisor_deadline" : "unexpected", exit.stage, exit.code, pass);
    } else if (!child_ok) emit(test, "child_failed", exit.stage, exit.code, false);
    else emit(test, observed, "validate", code, pass);
    frame_close(&first); frame_close(&next); pin_close(&pin);
    if (!drop_receive(&receive)) return false;
    return pass;
}
// Fault controls run in their own disposable process, never against real PIDs.
static int fault_mode, fault_waits;
static int fault_kill(pid_t child, int signal_number) {
    (void)child; (void)signal_number;
    if (fault_mode == 0) { errno=EPERM; return -1; }
    if (fault_mode == 1 || fault_mode == 3) { errno=ESRCH; return -1; }
    return 0;
}
static pid_t fault_wait(pid_t child, int *status, int flags) {
    (void)flags; ++fault_waits;
    if (fault_mode == 1 || fault_mode == 2) { errno=ECHILD; return -1; }
    if (fault_mode == 5) return child + 1;
    if (fault_mode == 7 && fault_waits == 1) { errno=EINTR; return -1; }
    *status = fault_mode == 3 ? 0 : fault_mode == 6 ? (SIGSTOP << 8) | 0x7f : SIGKILL;
    return child;
}
static int reaper_controls(void) {
    const Exit expected[] = {{"kill", EPERM}, {"wait", ECHILD}, {"wait", ECHILD}, {"child_exit", 0},
        {"supervisor", SUPERVISOR_DEADLINE}, {"wait", WAIT_FAILED}, {"wait", WAIT_FAILED}, {"supervisor", SUPERVISOR_DEADLINE}};
    for (fault_mode=0; fault_mode<8; ++fault_mode) {
        fault_waits=0;
        Exit observed = reap_with(123, 0, fault_wait, fault_kill);
        if (strcmp(observed.stage, expected[fault_mode].stage) || observed.code != expected[fault_mode].code ||
            (fault_mode == 0 && fault_waits != 0) || (fault_mode == 7 && fault_waits != 2)) return SETUP_FAILED;
    }
    return 0;
}
static int control_failure(const char *stage, int code) {
    printf("{\"stage\":\"%s\",\"code\":%d}\n", stage, code);
    return SETUP_FAILED;
}
static int send_failure_control(bool once) {
    mach_port_t receive = MACH_PORT_NULL, reply = MACH_PORT_NULL;
    kern_return_t result = mach_port_allocate(mach_task_self(), MACH_PORT_RIGHT_RECEIVE, &receive);
    if (result != KERN_SUCCESS) return control_failure("receive_allocate", result);
    result = mach_port_allocate(mach_task_self(), MACH_PORT_RIGHT_RECEIVE, &reply);
    if (result != KERN_SUCCESS) return control_failure("reply_allocate", result);
    result = mach_port_insert_right(mach_task_self(), receive, receive, MACH_MSG_TYPE_MAKE_SEND);
    if (result != KERN_SUCCESS) return control_failure("insert_send", result);
    mach_port_limits_t limits = {.mpl_qlimit=1};
    result = mach_port_set_attributes(mach_task_self(), receive, MACH_PORT_LIMITS_INFO,
            (mach_port_info_t)&limits, MACH_PORT_LIMITS_INFO_COUNT);
    if (result != KERN_SUCCESS) return control_failure("queue_limit", result);
    mach_msg_header_t seed = {0};
    seed.msgh_bits=MACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, 0);
    seed.msgh_size=sizeof(seed); seed.msgh_remote_port=receive;
    result = mach_msg(&seed, MACH_SEND_MSG | MACH_SEND_TIMEOUT, sizeof(seed), 0, MACH_PORT_NULL, 100, MACH_PORT_NULL);
    if (result != MACH_MSG_SUCCESS) return control_failure("seed_send", result);
    mach_port_t destination = receive;
    if (once) {
        mach_msg_type_name_t type = 0;
        result = mach_port_extract_right(mach_task_self(), receive, MACH_MSG_TYPE_MAKE_SEND_ONCE, &destination, &type);
        if (result != KERN_SUCCESS) return control_failure("extract_right", result);
        if (type != MACH_MSG_TYPE_PORT_SEND_ONCE) return control_failure("extracted_type", (int)type);
        // Send-once messages bypass queue limits; even an inactive destination
        // can report success. Use a deliberately invalid identical destination/
        // reply name instead: MOVE_SEND_ONCE + MAKE_SEND_ONCE cannot alias.
    }
    unsigned char session[16] = {0};
    send_message(destination, once ? destination : reply, once, HELLO, 1, session, 0, 0);
    return control_failure("unexpected_send_success", 0);
}
int main(int argc, char **argv) {
    if (argc == 2 && !strcmp(argv[1], "reaper-controls")) return reaper_controls();
    if (argc == 2 && !strcmp(argv[1], "send-timeout-copy")) return send_failure_control(false);
    if (argc == 2 && !strcmp(argv[1], "send-error-once-header")) return send_failure_control(true);
    if (argc == 2 && !strcmp(argv[1], "cleanup-control")) {
        mach_port_t receive = MACH_PORT_NULL;
        if (mach_port_allocate(mach_task_self(), MACH_PORT_RIGHT_RECEIVE, &receive) != KERN_SUCCESS) return SETUP_FAILED;
        mach_port_t wrong = receive;
        drop_send(&wrong); // Releasing a receive-only right as send must fail.
        int wrong_error = cleanup_error;
        cleanup_error = 0;
        bool closed = drop_receive(&receive);
        return wrong_error == KERN_INVALID_RIGHT && closed && cleanup_error == 0 ? CLEANUP_REFUSED : SETUP_FAILED;
    }
    if (argc == 3 && !strcmp(argv[1], "lookup-missing") && name_valid(argv[2])) return lookup_missing(argv[2]);
    if (argc == 3 && !strcmp(argv[1], "forced-exit") && name_valid(argv[2])) {
        mach_port_t port = MACH_PORT_NULL;
        if (bootstrap_check_in(bootstrap_port, argv[2], &port) != BOOTSTRAP_SUCCESS) return SETUP_FAILED;
        kill(getpid(), SIGKILL); return SETUP_FAILED;
    }
    if (argc == 6 && !strcmp(argv[1], "worker")) return worker(argv[0], argv[2], argv[3], argv[4], (int32_t)strtol(argv[5], NULL, 10));
    if (argc != 3 || strcmp(argv[1], "probe") || strlen(argv[2]) != strlen("com.cryptex.m8.bootstrap.") + 24) return SETUP_FAILED;
    char checked_name[BOOTSTRAP_MAX_NAME_LEN]; snprintf(checked_name, sizeof(checked_name), "%s.0", argv[2]);
    if (!name_valid(checked_name) || !fixture_open()) return SETUP_FAILED;
    bool ok = true;
    ok &= flow(argv[0], argv[2], 0, "normal", "two_way_after_imposter", OK, true);
    ok &= flow(argv[0], argv[2], 1, "replay", "same_sender_replay", REPLAY, false);
    ok &= flow(argv[0], argv[2], 2, "wrong-session", "wrong_session", WRONG_SESSION, false);
    ok &= flow(argv[0], argv[2], 3, "short", "short_frame", BAD_LAYOUT, false);
    ok &= flow(argv[0], argv[2], 4, "complex", "complex_frame", BAD_LAYOUT, false);
    ok &= flow(argv[0], argv[2], 5, "exec", "same_pid_exec", WRONG_GENERATION, false);
    ok &= flow(argv[0], argv[2], 6, "queued", "queued_after_exit", NOT_LIVE, false);
    ok &= flow(argv[0], argv[2], 7, "normal", "fresh_child_session", OK, false);
    ok &= flow(argv[0], argv[2], 8, "silent", "supervisor_deadline_distinct", CHILD_DEADLINE, false);
    bool removed = true;
    for (int i=0; i<9; ++i) {
        char name[BOOTSTRAP_MAX_NAME_LEN]; snprintf(name, sizeof(name), "%s.%d", argv[2], i);
        char *args[] = {argv[0], "lookup-missing", name, NULL};
        pid_t child = spawn_fixed(argv[0], args);
        Exit exit = child > 0 ? reap(child, 4000) : (Exit){"spawn", spawn_error};
        removed &= !strcmp(exit.stage, "child_exit") && exit.code == 0;
    }
    emit("normal_close_namespace_removal", removed ? "namespace_removed" : "namespace_not_removed", "independent_lookup", removed ? BOOTSTRAP_UNKNOWN_SERVICE : NAMESPACE_PRESENT, removed);
    emit("owned_right_cleanup", cleanup_error ? "cleanup_error" : "cleanup_calls_succeeded", "cleanup", cleanup_error, cleanup_error == 0);
    CFRelease(fixture_hash); CFRelease(fixture_requirement);
    return ok && removed && !failures ? 0 : 2;
}
