// Standalone SDK probe: the same adapter, with interception confined to this TU.
// No control hook, fixture image or alternate launch policy enters the daemon.
#include <libproc.h>
#include <stdbool.h>
#include <sys/proc_info.h>
static int fixture_pidfdinfo(int, int, int, void *, int);
#define proc_pidfdinfo fixture_pidfdinfo
#include "../src/owned_process/macos_endpoint.c"
#undef proc_pidfdinfo
#include <arpa/inet.h>
#include <stdio.h>
#include <sys/wait.h>

static const char credential[] = "SYNTHETIC_ENDPOINT_CREDENTIAL";
static const char response[] = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}";
static char executable[4096];
typedef struct { int port, fd; } Message;
typedef struct { pid_t pid; int control; cx_transport_pin *pin; bool reaped; } Worker;
static Worker workers[3];
static size_t worker_count;
static pid_t injection_pid;
static int injection_fd = -1, injection_control = -1, injection_reads;
static bool inject_reuse;

static void fail(const char *stage) {
    fprintf(stderr, "mac_endpoint_fixture_failed=%s\n", stage);
    exit(2);
}
#define REQUIRE(value, stage) do { if (!(value)) fail(stage); } while (false)
static void transfer(int fd, void *bytes, size_t size, bool writing) {
    unsigned char *cursor = bytes;
    while (size) {
        ssize_t n = writing ? write(fd, cursor, size) : read(fd, cursor, size);
        if (n < 0 && errno == EINTR) continue;
        REQUIRE(n > 0, "control_io");
        cursor += n;
        size -= (size_t)n;
    }
}
static void timeout(int fd) {
    struct timeval value = {.tv_sec = 2};
    REQUIRE(setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &value, sizeof(value)) == 0, "read_bound");
    REQUIRE(setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &value, sizeof(value)) == 0, "write_bound");
}
static void command(int control, char value, char expected) {
    transfer(control, &value, 1, true);
    transfer(control, &value, 1, false);
    REQUIRE(value == expected, "worker_ack");
}
static int fixture_pidfdinfo(int pid, int fd, int flavor, void *buffer, int size) {
    int result = proc_pidfdinfo(pid, fd, flavor, buffer, size);
    if (inject_reuse && pid == injection_pid && fd == injection_fd &&
        flavor == PROC_PIDFDSOCKETINFO && result == (int)sizeof(struct socket_fdinfo) &&
        ++injection_reads == 2) {
        // Return the original second snapshot, AFTER the actual child has
        // replaced that FD. Its duplicated original socket stays live, so a
        // rescan accepting a different matching FD would incorrectly succeed.
        command(injection_control, 'C', 'c');
        inject_reuse = false;
    }
    return result;
}
static void cleanup(void) {
    bool ok = true;
    for (size_t i = 0; i < worker_count; ++i) {
        Worker *worker = &workers[i];
        if (worker->pin && cx_transport_pin_close(worker->pin)) ok = false;
        if (worker->control >= 0 && close(worker->control)) ok = false;
        if (!worker->reaped) {
            // Only the exact unreaped child captured by this posix_spawn.
            if (kill(worker->pid, SIGKILL) && errno != ESRCH) ok = false;
            int status;
            pid_t got;
            do { got = waitpid(worker->pid, &status, 0); } while (got < 0 && errno == EINTR);
            if (got != worker->pid) ok = false;
            worker->reaped = true;
        }
    }
    if (!ok) _Exit(91);
}
static Worker *spawn_worker(const char *role, const uint8_t hash[20]) {
    REQUIRE(worker_count < 3, "child_limit");
    int pair[2];
    REQUIRE(socketpair(AF_UNIX, SOCK_STREAM, 0, pair) == 0, "control_pair");
    timeout(pair[0]);
    timeout(pair[1]);
    posix_spawnattr_t attr;
    posix_spawn_file_actions_t actions;
    REQUIRE(posix_spawnattr_init(&attr) == 0, "spawn_attributes");
    REQUIRE(posix_spawn_file_actions_init(&actions) == 0, "spawn_actions");
    sigset_t mask, defaults;
    sigemptyset(&mask);
    sigemptyset(&defaults);
    sigaddset(&defaults, SIGPIPE);
    REQUIRE(posix_spawnattr_setflags(&attr, POSIX_SPAWN_START_SUSPENDED |
        POSIX_SPAWN_CLOEXEC_DEFAULT | POSIX_SPAWN_SETSIGMASK | POSIX_SPAWN_SETSIGDEF) == 0, "spawn_flags");
    REQUIRE(posix_spawnattr_setsigmask(&attr, &mask) == 0, "spawn_mask");
    REQUIRE(posix_spawnattr_setsigdefault(&attr, &defaults) == 0, "spawn_defaults");
    for (int fd = 0; fd < 3; ++fd) {
        REQUIRE(posix_spawn_file_actions_addopen(&actions, fd, "/dev/null", O_RDWR, 0) == 0, "spawn_stdio");
    }
    REQUIRE(posix_spawn_file_actions_adddup2(&actions, pair[1], 3) == 0, "spawn_control");
    char *args[] = {executable, "--worker", (char *)role, NULL};
    char *env[] = {NULL};
    pid_t pid = 0;
    int result = posix_spawn(&pid, executable, &actions, &attr, args, env);
    // Register the captured child before any further operation can fail.
    REQUIRE(result == 0 && pid > 1, "captured_spawn");
    Worker *worker = &workers[worker_count++];
    *worker = (Worker){.pid = pid, .control = pair[0]};
    REQUIRE(posix_spawn_file_actions_destroy(&actions) == 0 && posix_spawnattr_destroy(&attr) == 0, "spawn_cleanup");
    REQUIRE(close(pair[1]) == 0, "control_close");
    REQUIRE(cx_transport_retain_child(pid, hash, &worker->pin) == 0, "capture_stopped_image");
    REQUIRE(kill(pid, SIGCONT) == 0, "resume_captured_image");
    return worker;
}

static void serve(const char *role, int inherited) {
    int server = inherited, observer = -1, listener = -1;
    timeout(3);
    if (inherited >= 0) {
        char ack = 'e';
        transfer(3, &ack, 1, true);
    } else if (!strcmp(role, "idle")) {
        Message ready = {0};
        transfer(3, &ready, sizeof(ready), true);
    } else {
        bool ipv6 = !strcmp(role, "v6");
        listener = socket(ipv6 ? AF_INET6 : AF_INET, SOCK_STREAM, 0);
        REQUIRE(listener >= 0, "server_socket");
        if (ipv6) {
            struct sockaddr_in6 address = {.sin6_len = sizeof(address), .sin6_family = AF_INET6, .sin6_addr = IN6ADDR_LOOPBACK_INIT};
            REQUIRE(bind(listener, (struct sockaddr *)&address, sizeof(address)) == 0, "server_bind_v6");
        } else {
            struct sockaddr_in address = {.sin_len = sizeof(address), .sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
            REQUIRE(bind(listener, (struct sockaddr *)&address, sizeof(address)) == 0, "server_bind_v4");
        }
        REQUIRE(listen(listener, 1) == 0, "server_listen");
        struct sockaddr_storage address;
        socklen_t bytes = sizeof(address);
        REQUIRE(getsockname(listener, (struct sockaddr *)&address, &bytes) == 0, "server_address");
        Message ready = {.port = ntohs(ipv6 ? ((struct sockaddr_in6 *)&address)->sin6_port : ((struct sockaddr_in *)&address)->sin_port), .fd = -1};
        transfer(3, &ready, sizeof(ready), true);
    }
    for (;;) {
        char op;
        ssize_t n = read(3, &op, 1);
        if (!n) _Exit(0);
        REQUIRE(n == 1, "worker_control");
        if (op == 'A') {
            server = accept(listener, NULL, NULL);
            REQUIRE(server >= 0, "server_accept");
            timeout(server);
            Message ready = {.fd = server};
            transfer(3, &ready, sizeof(ready), true);
        } else if (op == 'C') {
            observer = dup(server);
            REQUIRE(observer >= 0 && close(server) == 0, "duplicate_original_socket");
            int replacement = socket(AF_INET, SOCK_STREAM, 0);
            REQUIRE(replacement >= 0, "replacement_socket");
            if (replacement != server) {
                REQUIRE(dup2(replacement, server) == server && close(replacement) == 0, "replace_exact_fd");
            }
            op = 'c';
            transfer(3, &op, 1, true);
        } else if (op == 'D') {
            char bytes[64];
            REQUIRE(recv(observer >= 0 ? observer : server, bytes, sizeof(bytes), MSG_DONTWAIT) == -1 &&
                (errno == EAGAIN || errno == EWOULDBLOCK), "unexpected_application_bytes");
            op = 'z';
            transfer(3, &op, 1, true);
        } else if (op == 'N' || op == 'Q' || op == 'S' || op == 'E') {
            if (op != 'E') {
                char bytes[sizeof(credential)];
                transfer(server, bytes, sizeof(bytes), false);
                REQUIRE(!memcmp(bytes, credential, sizeof(bytes)), "synthetic_credential");
                transfer(server, (void *)response, sizeof(response) - 1, true);
            }
            if (op == 'N') {
                REQUIRE(close(server) == 0, "normal_http_close");
                op = 'n';
                transfer(3, &op, 1, true);
            } else if (op == 'S') {
                _Exit(0);
            } else {
                // Deliberately preserve only fixture-owned control/accepted FD
                // across same-image exec. No engine flags or binary are changed.
                REQUIRE(fcntl(3, F_SETFD, 0) == 0 && fcntl(server, F_SETFD, 0) == 0, "exec_fixture_fds");
                char text[32];
                snprintf(text, sizeof(text), "%d", server);
                char *args[] = {executable, "--after-exec", text, NULL};
                execv(executable, args);
                fail("same_pid_exec");
            }
        } else fail("unknown_control");
    }
}

int main(int argc, char **argv) {
    REQUIRE(realpath(argv[0], executable) != NULL, "image_path");
    if (argc == 3 && !strcmp(argv[1], "--worker")) {
        serve(argv[2], -1);
        return 2;
    }
    if (argc == 3 && !strcmp(argv[1], "--after-exec")) {
        char *end = NULL;
        long fd = strtol(argv[2], &end, 10);
        REQUIRE(end && !*end && fd > 3 && fd < 4096, "inherited_fd");
        serve("after", (int)fd);
        return 2;
    }
    REQUIRE(argc == 2 && getpgrp() == getpid() && getsid(0) == getpid(), "private_fixture_session");
    REQUIRE(atexit(cleanup) == 0, "cleanup_registration");
    signal(SIGPIPE, SIG_IGN);
    alarm(12);
    const char *cases[] = {"ipv4_close", "ipv6_close", "listener_only", "fd_reuse", "pre_send_exec", "queued_exec", "root_loss", "shutdown_exit"};
    bool known = false;
    for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) known |= !strcmp(argv[1], cases[i]);
    REQUIRE(known, "case_selection");
    uint8_t hash[20];
    REQUIRE(cx_transport_image_hash(executable, hash) == 0, "fixture_signature");
    Worker *root = spawn_worker(!strcmp(argv[1], "root_loss") ? "idle" : !strcmp(argv[1], "ipv6_close") ? "v6" : "v4", hash);
    Message ready;
    transfer(root->control, &ready, sizeof(ready), false);
    Worker *server = root;
    if (!strcmp(argv[1], "root_loss")) {
        server = spawn_worker("v4", hash);
        transfer(server->control, &ready, sizeof(ready), false);
    }
    REQUIRE(ready.port > 0 && ready.port <= 65535, "backend_port");
    bool ipv6 = !strcmp(argv[1], "ipv6_close");
    int client = socket(ipv6 ? AF_INET6 : AF_INET, SOCK_STREAM, 0);
    REQUIRE(client >= 0, "client_socket");
    timeout(client);
    if (ipv6) {
        struct sockaddr_in6 address = {.sin6_len = sizeof(address), .sin6_family = AF_INET6, .sin6_addr = IN6ADDR_LOOPBACK_INIT, .sin6_port = htons((uint16_t)ready.port)};
        REQUIRE(connect(client, (struct sockaddr *)&address, sizeof(address)) == 0, "connect_v6");
    } else {
        struct sockaddr_in address = {.sin_len = sizeof(address), .sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK), .sin_port = htons((uint16_t)ready.port)};
        REQUIRE(connect(client, (struct sockaddr *)&address, sizeof(address)) == 0, "connect_v4");
    }
    cx_transport_pin *peer = NULL;
    if (!strcmp(argv[1], "listener_only")) {
        REQUIRE(cx_transport_socket_peer(root->pin, client, &peer) == 1 && !peer, "listener_refused");
    } else {
        char op = 'A';
        transfer(server->control, &op, 1, true);
        transfer(server->control, &ready, sizeof(ready), false);
        if (!strcmp(argv[1], "fd_reuse")) {
            injection_pid = server->pid;
            injection_fd = ready.fd;
            injection_control = server->control;
            inject_reuse = true;
            REQUIRE(cx_transport_socket_peer(root->pin, client, &peer) == 4 && !peer &&
                !inject_reuse && injection_reads >= 2, "exact_original_witness_refused");
            command(server->control, 'D', 'z');
        } else {
            REQUIRE(cx_transport_socket_peer(root->pin, client, &peer) == 0 && peer, "coherent_socket_pin");
            REQUIRE(cx_transport_peer_validate(root->pin, peer, 1) == 0, "fresh_pre_send");
            if (!strcmp(argv[1], "pre_send_exec")) {
                command(server->control, 'E', 'e');
                REQUIRE(cx_transport_peer_validate(root->pin, peer, 1) == 3, "old_exec_generation_refused");
                command(server->control, 'D', 'z');
            } else if (!strcmp(argv[1], "root_loss")) {
                REQUIRE(kill(root->pid, SIGKILL) == 0, "kill_captured_root");
                int status;
                REQUIRE(waitpid(root->pid, &status, 0) == root->pid && WIFSIGNALED(status) && WTERMSIG(status) == SIGKILL, "root_exit");
                root->reaped = true;
                REQUIRE(cx_transport_pin_live(server->pin) && cx_transport_peer_validate(root->pin, peer, 1) == 3, "root_loss_refused_live_peer");
                command(server->control, 'D', 'z');
            } else {
                transfer(client, (void *)credential, sizeof(credential), true);
                op = !strcmp(argv[1], "queued_exec") ? 'Q' : !strcmp(argv[1], "shutdown_exit") ? 'S' : 'N';
                transfer(server->control, &op, 1, true);
                char bytes[sizeof(response) - 1];
                transfer(client, bytes, sizeof(bytes), false);
                REQUIRE(!memcmp(bytes, response, sizeof(bytes)), "complete_http_body");
                if (op == 'S') {
                    int status;
                    REQUIRE(waitpid(server->pid, &status, 0) == server->pid && WIFEXITED(status) && !WEXITSTATUS(status), "clean_ack_exit");
                    server->reaped = true;
                    REQUIRE(!cx_transport_pin_live(peer), "shutdown_is_not_live_identity");
                } else {
                    char ack;
                    transfer(server->control, &ack, 1, false);
                    REQUIRE(ack == (op == 'Q' ? 'e' : 'n'), "post_body_transition");
                    REQUIRE(cx_transport_peer_validate(root->pin, peer, 0) == (op == 'Q' ? 3 : 0), "response_generation_gate");
                }
            }
        }
    }
    if (peer) REQUIRE(cx_transport_pin_close(peer) == 0, "peer_right_cleanup");
    REQUIRE(close(client) == 0, "client_cleanup");
    printf("mac_transport_endpoint_%s=passed\n", argv[1]);
    return 0;
}
