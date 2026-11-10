// Synthetic RAW MACH actual-sender proof, not an XPC or production transport.
#include <Security/Security.h>
#include <bsm/libbsm.h>
#include <errno.h>
#include <fcntl.h>
#include <mach/mach.h>
#include <poll.h>
#include <spawn.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

extern char **environ;

typedef struct {
    mach_msg_header_t header;
    uint64_t sequence;
    audit_token_t claimed; // Deliberately untrusted payload; never used for auth.
} ProbeMessage;

typedef union {
    uint64_t align;
    unsigned char bytes[1024];
} ReceiveBuffer;

typedef struct {
    pid_t pid;
    int commands;
    audit_token_t token;
} Worker;

static int failures;
static SecRequirementRef requirement;
static audit_token_t pinned;
static pid_t pinned_pid;
static uint64_t last_sequence;
static mach_port_t pinned_task = MACH_PORT_NULL;

static void fail(const char *code) {
    fprintf(stderr, "probe_failure=%s\n", code);
    exit(2);
}

static void io_exact(int fd, void *data, size_t size, bool writing) {
    unsigned char *bytes = data;
    while (size) {
        struct pollfd wait = {.fd = fd, .events = writing ? POLLOUT : POLLIN};
        if (poll(&wait, 1, 3000) != 1) fail("pipe_deadline");
        ssize_t count = writing ? write(fd, bytes, size) : read(fd, bytes, size);
        if (count < 0 && errno == EINTR) continue;
        if (count <= 0) fail("pipe_short_io");
        bytes += count;
        size -= (size_t)count;
    }
}

static bool code_valid(const audit_token_t *token, pid_t pid) {
    CFDataRef audit = CFDataCreate(NULL, (const UInt8 *)token, sizeof(*token));
    CFNumberRef number = CFNumberCreate(NULL, kCFNumberIntType, &pid);
    const void *keys[] = {kSecGuestAttributeAudit, kSecGuestAttributePid};
    const void *values[] = {audit, number};
    CFDictionaryRef attributes = CFDictionaryCreate(NULL, keys, values, 2,
        &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    SecCodeRef code = NULL;
    OSStatus status = SecCodeCopyGuestWithAttributes(NULL, attributes, kSecCSDefaultFlags, &code);
    if (status == errSecSuccess && code) {
        status = SecCodeCheckValidity(code, kSecCSStrictValidate, requirement);
    } else if (status == errSecSuccess) {
        status = errSecCSInternalError;
    }
    if (code) CFRelease(code);
    CFRelease(attributes);
    CFRelease(number);
    CFRelease(audit);
    return status == errSecSuccess;
}

static void enroll(Worker worker) {
    // The token pipe is only a carrier. Kernel lookup validates both the PID
    // captured by posix_spawn and the opaque child token before trusting it.
    if (audit_token_to_pid(worker.token) != worker.pid ||
        !code_valid(&worker.token, worker.pid)) fail("enrollment_kernel_check");
    if (MACH_PORT_VALID(pinned_task)) mach_port_deallocate(mach_task_self(), pinned_task);
    pinned_task = MACH_PORT_NULL;
    if (task_name_for_pid(mach_task_self(), worker.pid, &pinned_task) != KERN_SUCCESS ||
        !MACH_PORT_VALID(pinned_task) || !code_valid(&worker.token, worker.pid)) fail("retained_instance_unavailable");
    pinned = worker.token;
    pinned_pid = worker.pid;
    last_sequence = 0;
}

static int worker_main(const char *mode, const char *binary) {
    mach_port_array_t ports = NULL;
    mach_msg_type_number_t count = 0;
    if (mach_ports_lookup(mach_task_self(), &ports, &count) != KERN_SUCCESS || count < 1 || !MACH_PORT_VALID(ports[0])) {
        fail("registered_send_right_unavailable");
    }
    mach_port_t send = ports[0];
    for (mach_msg_type_number_t i = 1; i < count; ++i) {
        if (MACH_PORT_VALID(ports[i])) mach_port_deallocate(mach_task_self(), ports[i]);
    }
    vm_deallocate(mach_task_self(), (vm_address_t)ports, count * sizeof(mach_port_t));
    if (strcmp(mode, "worker") == 0) {
        audit_token_t token;
        mach_msg_type_number_t words = TASK_AUDIT_TOKEN_COUNT;
        if (task_info(mach_task_self(), TASK_AUDIT_TOKEN, (task_info_t)&token, &words) != KERN_SUCCESS ||
            words != TASK_AUDIT_TOKEN_COUNT) fail("self_audit_unavailable");
        io_exact(4, &token, sizeof(token), true);
        close(4); // Exact one-shot enrollment pipe; no inheritance after exec.
    }
    for (;;) {
        unsigned char op;
        io_exact(3, &op, 1, false);
        if (op == 'q') return 0;
        if (op == 'e') {
            // Deliberate same-PID replacement. Only the synthetic control FD is
            // kept; the enrollment pipe was already closed.
            if (fcntl(3, F_SETFD, 0) < 0) fail("exec_control_fd");
            execl(binary, binary, "worker-exec", (char *)NULL);
            fail("self_exec_failed");
        }
        if (op != 's') fail("worker_command_invalid");
        ProbeMessage message = {0};
        io_exact(3, &message.sequence, sizeof(message.sequence), false);
        io_exact(3, &message.claimed, sizeof(message.claimed), false);
        message.header.msgh_bits = MACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, 0);
        message.header.msgh_size = sizeof(message);
        message.header.msgh_remote_port = send;
        message.header.msgh_id = 0x43585031;
        if (mach_msg(&message.header, MACH_SEND_MSG | MACH_SEND_TIMEOUT, sizeof(message),
                     0, MACH_PORT_NULL, 3000, MACH_PORT_NULL) != MACH_MSG_SUCCESS) fail("send_failed");
    }
}

static Worker spawn_worker(const char *binary) {
    int input[2], token[2];
    if (pipe(input) || pipe(token)) fail("pipe_create");
    // Keep file-action source FDs clear of fixed child destinations3/4.
    int sources[4];
    int originals[] = {input[0], input[1], token[0], token[1]};
    for (unsigned i = 0; i < 4; ++i) {
        sources[i] = fcntl(originals[i], F_DUPFD_CLOEXEC, 10);
        if (sources[i] < 0) fail("pipe_duplicate");
        close(originals[i]);
    }
    posix_spawn_file_actions_t actions;
    posix_spawnattr_t attrs;
    if (posix_spawn_file_actions_init(&actions) || posix_spawnattr_init(&attrs) ||
        posix_spawnattr_setflags(&attrs, POSIX_SPAWN_CLOEXEC_DEFAULT) ||
        posix_spawn_file_actions_adddup2(&actions, sources[0], 3) ||
        posix_spawn_file_actions_adddup2(&actions, sources[3], 4) ||
        posix_spawn_file_actions_adddup2(&actions, STDOUT_FILENO, STDOUT_FILENO) ||
        posix_spawn_file_actions_adddup2(&actions, STDERR_FILENO, STDERR_FILENO)) fail("spawn_setup");
    Worker worker = {0};
    char *args[] = {(char *)binary, "worker", NULL};
    int error = posix_spawn(&worker.pid, binary, &actions, &attrs, args, environ);
    posix_spawn_file_actions_destroy(&actions);
    posix_spawnattr_destroy(&attrs);
    close(sources[0]);
    close(sources[3]);
    if (error) fail("worker_spawn");
    worker.commands = sources[1];
    io_exact(sources[2], &worker.token, sizeof(worker.token), false);
    unsigned char extra;
    struct pollfd ready = {.fd = sources[2], .events = POLLIN};
    if (poll(&ready, 1, 3000) != 1 || read(sources[2], &extra, 1) != 0) fail("enrollment_extra_or_unclosed");
    close(sources[2]);
    return worker;
}

static void send_case(Worker worker, uint64_t sequence, const audit_token_t *claimed) {
    unsigned char op = 's';
    io_exact(worker.commands, &op, 1, true);
    io_exact(worker.commands, &sequence, sizeof(sequence), true);
    audit_token_t value = {0};
    if (claimed) value = *claimed;
    io_exact(worker.commands, &value, sizeof(value), true);
}

static void stop_worker(Worker worker) {
    unsigned char op = 'q';
    io_exact(worker.commands, &op, 1, true);
    close(worker.commands);
    int status;
    if (waitpid(worker.pid, &status, 0) != worker.pid || !WIFEXITED(status) || WEXITSTATUS(status) != 0) fail("worker_exit");
}

static const char *receive_case(mach_port_t port, bool *same_pid, bool *same_version) {
    ReceiveBuffer buffer = {0};
    mach_msg_header_t *header = (mach_msg_header_t *)buffer.bytes;
    mach_msg_option_t options = MACH_RCV_MSG | MACH_RCV_TIMEOUT |
        MACH_RCV_TRAILER_TYPE(MACH_MSG_TRAILER_FORMAT_0) | MACH_RCV_TRAILER_ELEMENTS(MACH_RCV_TRAILER_AUDIT);
    if (mach_msg(header, options, 0, sizeof(buffer), port, 3000, MACH_PORT_NULL) != MACH_MSG_SUCCESS) fail("receive_failed");
    if (header->msgh_bits & MACH_MSGH_BITS_COMPLEX || header->msgh_size != sizeof(ProbeMessage) ||
        header->msgh_id != 0x43585031 || header->msgh_remote_port != MACH_PORT_NULL ||
        header->msgh_voucher_port != MACH_PORT_NULL) {
        mach_msg_destroy(header);
        fail("message_layout_invalid");
    }
    size_t offset = (header->msgh_size + 3u) & ~(size_t)3u;
    if (offset > sizeof(buffer) || sizeof(mach_msg_audit_trailer_t) > sizeof(buffer) - offset) fail("trailer_bounds");
    mach_msg_audit_trailer_t trailer;
    memcpy(&trailer, buffer.bytes + offset, sizeof(trailer));
    if (trailer.msgh_trailer_type != MACH_MSG_TRAILER_FORMAT_0 ||
        trailer.msgh_trailer_size != sizeof(trailer) || trailer.msgh_trailer_size > sizeof(buffer) - offset) fail("audit_trailer_missing");
    // Access only Apple's public audit-token getters, never token slots.
    *same_pid = audit_token_to_pid(trailer.msgh_audit) == pinned_pid;
    *same_version = audit_token_to_pidversion(trailer.msgh_audit) == audit_token_to_pidversion(pinned);
    if (!*same_pid || !*same_version) return "sender_instance_rejected";
    mach_port_type_t type = 0;
    pid_t live_pid = 0;
    if (mach_port_type(mach_task_self(), pinned_task, &type) != KERN_SUCCESS ||
        !(type & MACH_PORT_TYPE_SEND) || type & MACH_PORT_TYPE_DEAD_NAME ||
        pid_for_task(pinned_task, &live_pid) != KERN_SUCCESS || live_pid != pinned_pid ||
        !code_valid(&pinned, pinned_pid) || !code_valid(&trailer.msgh_audit, pinned_pid)) return "sender_not_live_or_code_invalid";
    ProbeMessage message;
    memcpy(&message, buffer.bytes, sizeof(message));
    if (!message.sequence || message.sequence <= last_sequence) return "replay_rejected";
    last_sequence = message.sequence;
    return "accepted";
}

static void observe(mach_port_t port, const char *name, const char *expected,
                    int expected_pid, int expected_version) {
    bool same_pid, same_version;
    const char *result = receive_case(port, &same_pid, &same_version);
    bool passed = strcmp(result, expected) == 0 &&
        (expected_pid < 0 || same_pid == (bool)expected_pid) &&
        (expected_version < 0 || same_version == (bool)expected_version);
    printf("{\"case\":\"%s\",\"result\":\"%s\",\"same_pid\":%s,\"same_generation\":%s,\"pass\":%s}\n",
           name, result, same_pid ? "true" : "false", same_version ? "true" : "false", passed ? "true" : "false");
    if (!passed) ++failures;
}

int main(int argc, const char **argv) {
    signal(SIGPIPE, SIG_IGN);
    if (argc == 2 && (strcmp(argv[1], "worker") == 0 || strcmp(argv[1], "worker-exec") == 0)) {
        return worker_main(argv[1], argv[0]);
    }
    if (argc != 3 || strcmp(argv[1], "probe") != 0) return 2;
    CFStringRef text = CFStringCreateWithCString(NULL, argv[2], kCFStringEncodingUTF8);
    if (!text || SecRequirementCreateWithString(text, kSecCSDefaultFlags, &requirement) != errSecSuccess) fail("requirement_invalid");
    CFRelease(text);
    mach_port_t receive;
    if (mach_port_allocate(mach_task_self(), MACH_PORT_RIGHT_RECEIVE, &receive) != KERN_SUCCESS ||
        mach_port_insert_right(mach_task_self(), receive, receive, MACH_MSG_TYPE_MAKE_SEND) != KERN_SUCCESS) fail("port_create");
    // Only this disposable test process changes its inherited registration list.
    mach_port_array_t previous = NULL;
    mach_msg_type_number_t previous_count = 0;
    if (mach_ports_lookup(mach_task_self(), &previous, &previous_count) != KERN_SUCCESS ||
        mach_ports_register(mach_task_self(), &receive, 1) != KERN_SUCCESS) fail("private_registration");
    Worker expected = spawn_worker(argv[0]);
    Worker other = spawn_worker(argv[0]);
    if (mach_ports_register(mach_task_self(), previous, previous_count) != KERN_SUCCESS) fail("registration_restore");
    for (mach_msg_type_number_t i = 0; i < previous_count; ++i) {
        if (MACH_PORT_VALID(previous[i])) mach_port_deallocate(mach_task_self(), previous[i]);
    }
    if (previous) vm_deallocate(mach_task_self(), (vm_address_t)previous, previous_count * sizeof(mach_port_t));
    enroll(expected);
    if (!code_valid(&other.token, other.pid)) fail("negative_fixture_code_invalid");
    if (code_valid(&expected.token, other.pid)) fail("forged_enrollment_accepted");
    puts("{\"case\":\"forged_enrollment_pid_token_pair\",\"result\":\"rejected\",\"pass\":true}");
    send_case(expected, 1, NULL);
    observe(receive, "expected_sender", "accepted", 1, 1);
    send_case(other, 2, &expected.token);
    observe(receive, "same_binary_shared_send_right_forged_payload", "sender_instance_rejected", 0, -1);
    send_case(expected, 1, NULL);
    observe(receive, "same_sender_replay", "replay_rejected", 1, 1);
    send_case(expected, 2, NULL);
    observe(receive, "expected_after_negative", "accepted", 1, 1);
    unsigned char exec = 'e';
    io_exact(expected.commands, &exec, 1, true);
    send_case(expected, 3, &pinned);
    observe(receive, "same_pid_exec_old_epoch", "sender_instance_rejected", 1, 0);
    stop_worker(expected);
    // A new explicitly enrolled execution succeeds on the same receive endpoint.
    // Other is already live and signature-validated, but was never the prior pin.
    enroll(other);
    send_case(other, 1, NULL);
    observe(receive, "fresh_enrolled_execution", "accepted", 1, 1);
    send_case(other, 2, NULL);
    stop_worker(other); // Reap before receiving the queued old-generation message.
    observe(receive, "queued_message_after_sender_exit", "sender_not_live_or_code_invalid", 1, 1);
    mach_port_destroy(mach_task_self(), receive);
    mach_port_deallocate(mach_task_self(), pinned_task);
    CFRelease(requirement);
    return failures ? 1 : 0;
}
