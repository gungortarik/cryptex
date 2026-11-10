// Bounded candidate test only. Not linked into Desktop/daemon or owner IPC.
// Trusted launched TCB -> verified self organization -> sealed counterpart.
#include <Security/Security.h>
#include <AvailabilityMacros.h>
#include <bsm/libbsm.h>
#include <errno.h>
#include <fcntl.h>
#include <libproc.h>
#include <mach/mach.h>
#include <poll.h>
#include <signal.h>
#include <spawn.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

extern char **environ;
#define DESKTOP_ID "com.cryptex.m8.custody.desktop"
#define DAEMON_ID "com.cryptex.m8.custody.daemon"
#define HANDOFF_FD 198
#define REFUSED 40
#define MALFORMED 41
#define DEADLINE 42
#define INTERNAL 43
_Static_assert(sizeof(audit_token_t) == 32, "opaque handoff size changed");
static const char *policy_diagnostic = "untrusted_self";

// Do not copy numeric flags or manually redeclare a missing SDK API.
#if defined(__MAC_27_0) && defined(O_CLOFORK) && defined(FD_CLOFORK) && MAC_OS_X_VERSION_MAX_ALLOWED >= __MAC_27_0
#define HAS_ATOMIC_API 1
#else
#define HAS_ATOMIC_API 0
#endif

typedef struct {
    SecRequirementRef peer;
    CFDataRef peer_hash;
    char bundle[4096];
    char desktop[4096];
    char daemon[4096];
} Policy;

static long long now_ms(void) {
    struct timespec time;
    if (clock_gettime(CLOCK_MONOTONIC, &time)) _exit(INTERNAL);
    return (long long)time.tv_sec * 1000 + time.tv_nsec / 1000000;
}

static void row(const char *name, const char *result, bool pass) {
    printf("{\"case\":\"%s\",\"result\":\"%s\",\"pass\":%s}\n", name, result, pass ? "true" : "false");
    fflush(stdout);
}

static SecRequirementRef requirement(const char *identifier, const char *team) {
    char text[512];
    if (snprintf(text, sizeof(text), "identifier \"%s\" and anchor apple generic and certificate leaf[subject.OU] = \"%s\"", identifier, team) >= (int)sizeof(text)) return NULL;
    CFStringRef string = CFStringCreateWithCString(NULL, text, kCFStringEncodingUTF8);
    SecRequirementRef result = NULL;
    if (string) {
        if (SecRequirementCreateWithString(string, kSecCSDefaultFlags, &result) != errSecSuccess) result = NULL;
        CFRelease(string);
    }
    return result;
}

static CFDataRef image_hash(SecStaticCodeRef code) {
    CFDictionaryRef info = NULL;
    if (SecCodeCopySigningInformation(code, kSecCSDefaultFlags, &info) != errSecSuccess || !info) return NULL;
    CFTypeRef hash = CFDictionaryGetValue(info, kSecCodeInfoUnique);
    CFDataRef result = hash && CFGetTypeID(hash) == CFDataGetTypeID() && CFDataGetLength(hash) > 0 ? CFRetain(hash) : NULL;
    CFRelease(info);
    return result;
}

static SecStaticCodeRef static_image(const char *path, SecRequirementRef expected, bool nested) {
    CFURLRef url = CFURLCreateFromFileSystemRepresentation(NULL, (const UInt8 *)path, strlen(path), false);
    SecStaticCodeRef code = NULL;
    SecCSFlags flags = kSecCSStrictValidate | kSecCSCheckAllArchitectures;
    if (nested) flags |= kSecCSCheckNestedCode;
    if (url && (SecStaticCodeCreateWithPath(url, kSecCSDefaultFlags, &code) != errSecSuccess ||
        !code || SecStaticCodeCheckValidity(code, flags, expected) != errSecSuccess)) {
        if (code) CFRelease(code);
        code = NULL;
    }
    if (url) CFRelease(url);
    return code;
}

static void policy_close(Policy *policy) {
    if (policy->peer) CFRelease(policy->peer);
    if (policy->peer_hash) CFRelease(policy->peer_hash);
    memset(policy, 0, sizeof(*policy));
}

static bool policy_open(Policy *policy, bool desktop) {
    memset(policy, 0, sizeof(*policy));
    const char *own_id = desktop ? DESKTOP_ID : DAEMON_ID;
    SecCodeRef self = NULL;
    CFDictionaryRef info = NULL;
    SecRequirementRef initial = NULL, own = NULL, bundle_req = NULL;
    SecStaticCodeRef bundle = NULL, disk_self = NULL, peer = NULL;
    CFDataRef live_hash = NULL, disk_hash = NULL;
    CFStringRef initial_text = CFStringCreateWithFormat(NULL, NULL, CFSTR("identifier \"%s\" and anchor apple generic"), own_id);
    bool valid = false;
    policy_diagnostic = "apple_anchor_or_fixed_self_identifier_refused";
    if (!initial_text || SecRequirementCreateWithString(initial_text, 0, &initial) != errSecSuccess ||
        SecCodeCopySelf(0, &self) != errSecSuccess || !self ||
        SecCodeCheckValidity(self, kSecCSStrictValidate, initial) != errSecSuccess ||
        SecCodeCopySigningInformation((SecStaticCodeRef)self, 0, &info) != errSecSuccess || !info) goto done;
    // Dynamic signing info is read only AFTER successful live-code validation.
    policy_diagnostic = "certified_team_unavailable";
    CFTypeRef team_value = CFDictionaryGetValue(info, kSecCodeInfoTeamIdentifier);
    char team[64];
    if (!team_value || CFGetTypeID(team_value) != CFStringGetTypeID() ||
        !CFStringGetCString(team_value, team, sizeof(team), kCFStringEncodingASCII) || strlen(team) != 10) goto done;
    for (size_t i = 0; i < 10; ++i) if (!((team[i] >= 'A' && team[i] <= 'Z') || (team[i] >= '0' && team[i] <= '9'))) goto done;
    own = requirement(own_id, team);
    bundle_req = requirement(DESKTOP_ID, team);
    policy->peer = requirement(desktop ? DAEMON_ID : DESKTOP_ID, team);
    if (!own || !bundle_req || !policy->peer || SecCodeCheckValidity(self, kSecCSStrictValidate, own) != errSecSuccess) goto done;
    live_hash = image_hash((SecStaticCodeRef)self);
    char path[4096] = {0};
    int path_size = proc_pidpath(getpid(), path, sizeof(path));
    policy_diagnostic = "packaged_self_path_refused";
    if (path_size <= 0 || path_size >= (int)sizeof(path)) goto done;
    const char *suffix = desktop ? "/Contents/MacOS/Desktop" : "/Contents/MacOS/Daemon";
    size_t size = strlen(path), tail = strlen(suffix);
    if (size <= tail || strcmp(path + size - tail, suffix) != 0 || size - tail >= sizeof(policy->bundle)) goto done;
    memcpy(policy->bundle, path, size - tail);
    policy->bundle[size - tail] = 0;
    if (snprintf(policy->desktop, sizeof(policy->desktop), "%s/Contents/MacOS/Desktop", policy->bundle) >= (int)sizeof(policy->desktop) ||
        snprintf(policy->daemon, sizeof(policy->daemon), "%s/Contents/MacOS/Daemon", policy->bundle) >= (int)sizeof(policy->daemon)) goto done;
    policy_diagnostic = "sealed_bundle_or_counterpart_refused";
    bundle = static_image(policy->bundle, bundle_req, true);
    disk_self = static_image(path, own, false);
    peer = static_image(desktop ? policy->daemon : policy->desktop, policy->peer, false);
    if (!bundle || !disk_self || !peer || !live_hash) goto done;
    disk_hash = image_hash(disk_self);
    policy->peer_hash = image_hash(peer);
    if (!disk_hash || !policy->peer_hash || !CFEqual(live_hash, disk_hash) ||
        SecCodeCheckValidity(self, kSecCSStrictValidate, own) != errSecSuccess) goto done;
    valid = true;
    policy_diagnostic = "verified_same_organization_bundle";
done:
    if (initial_text) CFRelease(initial_text);
    if (initial) CFRelease(initial);
    if (own) CFRelease(own);
    if (bundle_req) CFRelease(bundle_req);
    if (self) CFRelease(self);
    if (info) CFRelease(info);
    if (bundle) CFRelease(bundle);
    if (disk_self) CFRelease(disk_self);
    if (peer) CFRelease(peer);
    if (live_hash) CFRelease(live_hash);
    if (disk_hash) CFRelease(disk_hash);
    if (!valid) policy_close(policy);
    return valid;
}

static bool parent_valid(const audit_token_t *token, Policy *policy) {
    pid_t expected = getppid();
    if (expected <= 1 || audit_token_to_pid(*token) != expected) return false;
    CFDataRef audit = CFDataCreate(NULL, (const UInt8 *)token, sizeof(*token));
    CFNumberRef pid = CFNumberCreate(NULL, kCFNumberIntType, &expected);
    CFDictionaryRef attrs = NULL;
    SecCodeRef parent = NULL;
    CFDataRef hash = NULL;
    mach_port_t task = MACH_PORT_NULL;
    bool valid = false;
    if (!audit || !pid) goto done;
    const void *keys[] = {kSecGuestAttributeAudit, kSecGuestAttributePid};
    const void *values[] = {audit, pid};
    attrs = CFDictionaryCreate(NULL, keys, values, 2, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    if (!attrs || SecCodeCopyGuestWithAttributes(NULL, attrs, 0, &parent) != errSecSuccess || !parent ||
        SecCodeCheckValidity(parent, kSecCSStrictValidate, policy->peer) != errSecSuccess) goto done;
    hash = image_hash((SecStaticCodeRef)parent);
    if (!hash || !CFEqual(hash, policy->peer_hash) ||
        task_name_for_pid(mach_task_self(), expected, &task) != KERN_SUCCESS || !MACH_PORT_VALID(task)) goto done;
    mach_port_type_t type = 0;
    pid_t held_pid = 0;
    valid = getppid() == expected && mach_port_type(mach_task_self(), task, &type) == KERN_SUCCESS &&
        (type & MACH_PORT_TYPE_SEND) && !(type & MACH_PORT_TYPE_DEAD_NAME) &&
        pid_for_task(task, &held_pid) == KERN_SUCCESS && held_pid == expected &&
        SecCodeCheckValidity(parent, kSecCSStrictValidate, policy->peer) == errSecSuccess;
done:
    if (MACH_PORT_VALID(task)) mach_port_deallocate(mach_task_self(), task);
    if (hash) CFRelease(hash);
    if (parent) CFRelease(parent);
    if (attrs) CFRelease(attrs);
    if (pid) CFRelease(pid);
    if (audit) CFRelease(audit);
    return valid;
}

static bool atomic_available(void) {
#if HAS_ATOMIC_API
    if (__builtin_available(macOS 27.0, *)) return true;
#endif
    return false;
}

static bool atomic_pipe(int fds[2]) {
#if HAS_ATOMIC_API
    if (__builtin_available(macOS 27.0, *)) {
        if (pipe2(fds, O_CLOEXEC | O_CLOFORK | O_NONBLOCK)) return false;
        int expected = FD_CLOEXEC | FD_CLOFORK;
        if ((fcntl(fds[0], F_GETFD) & expected) == expected && (fcntl(fds[1], F_GETFD) & expected) == expected) return true;
        close(fds[0]); close(fds[1]);
    }
#else
    (void)fds;
#endif
    return false;
}

static int reap(pid_t pid, int timeout) {
    long long deadline = now_ms() + timeout;
    int status;
    while (now_ms() < deadline) {
        pid_t observed = waitpid(pid, &status, WNOHANG);
        if (observed == pid) return WIFEXITED(status) ? WEXITSTATUS(status) : INTERNAL;
        if (observed < 0 && errno != EINTR) return INTERNAL;
        struct timespec pause = {.tv_sec=0, .tv_nsec=10000000};
        nanosleep(&pause, NULL);
    }
    kill(pid, SIGKILL);
    while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {}
    return DEADLINE;
}

static pid_t spawn(const char *binary, char *const args[], int handoff) {
    posix_spawn_file_actions_t actions;
    posix_spawnattr_t attributes;
    if (posix_spawn_file_actions_init(&actions)) return -1;
    if (posix_spawnattr_init(&attributes)) { posix_spawn_file_actions_destroy(&actions); return -1; }
    int error = posix_spawnattr_setflags(&attributes, POSIX_SPAWN_CLOEXEC_DEFAULT);
    for (int fd = 0; !error && fd < 3; ++fd) error = posix_spawn_file_actions_adddup2(&actions, fd, fd);
    if (!error && handoff >= 0) error = posix_spawn_file_actions_adddup2(&actions, handoff, HANDOFF_FD);
    pid_t child = -1;
    if (!error) error = posix_spawn(&child, binary, &actions, &attributes, args, environ);
    posix_spawn_file_actions_destroy(&actions);
    posix_spawnattr_destroy(&attributes);
    return error ? -1 : child;
}

static bool fork_excludes(int first, int second) {
    pid_t child = fork();
    if (child < 0) return false;
    if (child == 0) {
        // After a multithreaded fork: only async-signal-safe syscalls.
        int a = fcntl(first, F_GETFD), a_error = errno;
        int b = second < 0 ? -1 : fcntl(second, F_GETFD), b_error = errno;
        _exit(a == -1 && a_error == EBADF && (second < 0 || (b == -1 && b_error == EBADF)) ? 0 : REFUSED);
    }
    return reap(child, 2000) == 0;
}

static int read_handoff(void) {
#if HAS_ATOMIC_API
    if (fcntl(HANDOFF_FD, F_SETFD, FD_CLOEXEC | FD_CLOFORK) < 0 || !fork_excludes(HANDOFF_FD, -1)) return INTERNAL;
#else
    return INTERNAL;
#endif
    unsigned char bytes[sizeof(audit_token_t) + 1];
    size_t used = 0;
    long long deadline = now_ms() + 2000;
    int result = DEADLINE;
    while (now_ms() < deadline) {
        struct pollfd event = {.fd=HANDOFF_FD, .events=POLLIN};
        if (poll(&event, 1, 50) < 0 && errno != EINTR) { result = MALFORMED; break; }
        ssize_t count = read(HANDOFF_FD, bytes + used, sizeof(bytes) - used);
        if (count < 0 && (errno == EINTR || errno == EAGAIN)) continue;
        if (count < 0) { result = MALFORMED; break; }
        if (!count) { result = used == sizeof(audit_token_t) ? 0 : MALFORMED; break; }
        used += (size_t)count;
        if (used > sizeof(audit_token_t)) { result = MALFORMED; break; }
    }
    close(HANDOFF_FD); // Before Security/framework use or any further spawn.
    if (result) { memset(bytes, 0, sizeof(bytes)); return result; }
    audit_token_t token;
    memcpy(&token, bytes, sizeof(token));
    memset(bytes, 0, sizeof(bytes));
    Policy policy;
    result = policy_open(&policy, false) && parent_valid(&token, &policy) ? 0 : REFUSED;
    policy_close(&policy);
    memset(&token, 0, sizeof(token));
    return result;
}

static bool handoff(const char *daemon, int mode, int expected) {
    int fds[2];
    if (!atomic_pipe(fds)) return false;
    audit_token_t token;
    mach_msg_type_number_t words = TASK_AUDIT_TOKEN_COUNT;
    bool valid = task_info(mach_task_self(), TASK_AUDIT_TOKEN, (task_info_t)&token, &words) == KERN_SUCCESS && words == TASK_AUDIT_TOKEN_COUNT;
    unsigned char bytes[sizeof(token) + 1];
    memcpy(bytes, &token, sizeof(token)); bytes[sizeof(token)] = 0;
    size_t size = sizeof(token);
    if (mode == 1) --size;
    if (mode == 2) ++size;
    if (mode == 3) memset(bytes, 0, sizeof(bytes));
    valid = valid && write(fds[1], bytes, size) == (ssize_t)size;
    memset(&token, 0, sizeof(token)); memset(bytes, 0, sizeof(bytes));
    if (mode != 4) close(fds[1]); // Prefill and EOF before intended spawn.
    char *args[] = {(char *)daemon, "handoff", NULL};
    pid_t child = valid ? spawn(daemon, args, fds[0]) : -1;
    close(fds[0]); // Drop parent copy immediately after spawn.
    int code = child > 0 ? reap(child, 5000) : INTERNAL;
    if (mode == 4) close(fds[1]);
    return code == expected;
}

int main(int argc, char **argv) {
    if (argc == 2 && !strcmp(argv[1], "handoff")) return read_handoff();
    if (argc == 4 && !strcmp(argv[1], "inspect-closed")) {
        int a = atoi(argv[2]), b = atoi(argv[3]);
        return fcntl(a, F_GETFD) < 0 && errno == EBADF && fcntl(b, F_GETFD) < 0 && errno == EBADF ? 0 : REFUSED;
    }
    if (argc != 4 || strcmp(argv[1], "probe")) return 2;
    Policy policy;
    bool eligible = policy_open(&policy, true);
    printf("{\"capability\":\"atomic_api\",\"headers\":%s,\"runtime\":%s}\n", HAS_ATOMIC_API ? "true" : "false", atomic_available() ? "true" : "false");
    printf("{\"capability\":\"self_anchored_bundle\",\"eligible\":%s,\"diagnostic\":\"%s\"}\n", eligible ? "true" : "false", policy_diagnostic);
    if (!eligible) {
        policy_close(&policy);
        return 0; // Explicitly incomplete, never reported as positive custody.
    }
    SecStaticCodeRef wrong = static_image(argv[2], policy.peer, false);
    row("wrong_identifier_refused", "refused", wrong == NULL);
    if (wrong) CFRelease(wrong);
    SecStaticCodeRef adhoc = static_image(argv[3], policy.peer, false);
    row("adhoc_counterpart_refused", "refused", adhoc == NULL);
    if (adhoc) CFRelease(adhoc);
    if (!atomic_available()) { policy_close(&policy); return 0; }
    int fds[2];
    if (!atomic_pipe(fds)) { policy_close(&policy); return 2; }
    row("raw_fork_excludes_both_ends", "excluded", fork_excludes(fds[0], fds[1]));
    char a[32], b[32]; snprintf(a, sizeof(a), "%d", fds[0]); snprintf(b, sizeof(b), "%d", fds[1]);
    char *inspect[] = {policy.desktop, "inspect-closed", a, b, NULL};
    pid_t other = spawn(policy.desktop, inspect, -1);
    row("unrelated_spawn_excludes_both_ends", "excluded", other > 0 && reap(other, 2000) == 0);
    close(fds[0]); close(fds[1]);
    row("intended_child_live_parent", "accepted", handoff(policy.daemon, 0, 0));
    row("short_handoff", "refused", handoff(policy.daemon, 1, MALFORMED));
    row("extra_handoff", "refused", handoff(policy.daemon, 2, MALFORMED));
    row("forged_parent_token", "refused", handoff(policy.daemon, 3, REFUSED));
    row("missing_eof", "deadline_refused", handoff(policy.daemon, 4, DEADLINE));
    row("fresh_child_restart", "accepted", handoff(policy.daemon, 0, 0));
    policy_close(&policy);
    return 0;
}
