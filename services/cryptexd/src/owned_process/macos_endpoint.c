// Default-off Mac transport candidate. Public SDK only; never a PID-adoption API.
#include <Security/Security.h>
#include <bsm/libbsm.h>
#include <errno.h>
#include <libproc.h>
#include <mach/mach.h>
#include <netinet/in.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <sys/proc.h>
#include <sys/proc_info.h>
#include <sys/socket.h>
#include <sys/acl.h>
#include <unistd.h>

typedef struct {
    pid_t pid;
    mach_port_t task;
    audit_token_t token;
    uint8_t hash[20];
} cx_transport_pin;

int cx_transport_private_acl(int fd) {
    errno = 0;
    acl_t acl = acl_get_fd_np(fd, ACL_TYPE_EXTENDED);
    if (!acl) return (errno == ENOENT || errno == ENOATTR) ? 0 : 1;
    acl_entry_t entry;
    int present = acl_get_entry(acl, ACL_FIRST_ENTRY, &entry);
    acl_free(acl);
    return present == 0 ? 0 : 1;
}

int cx_transport_pin_live(const cx_transport_pin *pin) {
    pid_t pid = 0;
    audit_token_t token;
    mach_msg_type_number_t count = TASK_AUDIT_TOKEN_COUNT;
    return pin && pid_for_task(pin->task, &pid) == KERN_SUCCESS && pid == pin->pid &&
        task_info(pin->task, TASK_AUDIT_TOKEN, (task_info_t)&token, &count) == KERN_SUCCESS &&
        count == TASK_AUDIT_TOKEN_COUNT && !memcmp(&token, &pin->token, sizeof(token));
}

int cx_transport_pin_close(cx_transport_pin *pin) {
    if (!pin) return 1;
    int result = MACH_PORT_VALID(pin->task) && mach_port_deallocate(mach_task_self(), pin->task) != KERN_SUCCESS;
    memset(pin, 0, sizeof(*pin));
    free(pin);
    return result;
}

static bool copy_hash(SecStaticCodeRef code, uint8_t out[20]) {
    CFDictionaryRef info = NULL;
    bool ok = false;
    if (SecCodeCopySigningInformation(code, kSecCSSigningInformation, &info) == errSecSuccess && info) {
        CFTypeRef hash = CFDictionaryGetValue(info, kSecCodeInfoUnique);
        if (hash && CFGetTypeID(hash) == CFDataGetTypeID() && CFDataGetLength(hash) == 20) {
            memcpy(out, CFDataGetBytePtr(hash), 20);
            ok = true;
        }
    }
    if (info) CFRelease(info);
    return ok;
}

int cx_transport_image_hash(const char *path, uint8_t out[20]) {
    CFURLRef url = CFURLCreateFromFileSystemRepresentation(NULL, (const UInt8 *)path, strlen(path), false);
    SecStaticCodeRef code = NULL;
    bool ok = url && SecStaticCodeCreateWithPath(url, 0, &code) == errSecSuccess && code &&
        SecStaticCodeCheckValidity(code, kSecCSStrictValidate | kSecCSCheckAllArchitectures, NULL) == errSecSuccess &&
        copy_hash(code, out);
    if (code) CFRelease(code);
    if (url) CFRelease(url);
    return ok ? 0 : 1;
}

static bool image_matches(const cx_transport_pin *pin) {
    if (!cx_transport_pin_live(pin)) return false;
    CFDataRef token = CFDataCreate(NULL, (const UInt8 *)&pin->token, sizeof(pin->token));
    if (!token) return false;
    const void *key = kSecGuestAttributeAudit;
    const void *value = token;
    CFDictionaryRef attributes = CFDictionaryCreate(NULL, &key, &value, 1,
        &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    SecCodeRef code = NULL;
    uint8_t hash[20];
    bool ok = attributes && SecCodeCopyGuestWithAttributes(NULL, attributes, 0, &code) == errSecSuccess &&
        code && SecCodeCheckValidity(code, kSecCSStrictValidate, NULL) == errSecSuccess &&
        copy_hash((SecStaticCodeRef)code, hash) && !memcmp(hash, pin->hash, 20) &&
        cx_transport_pin_live(pin);
    if (code) CFRelease(code);
    if (attributes) CFRelease(attributes);
    CFRelease(token);
    return ok;
}

// Caller supplies only a captured child, or a member of the helper's private
// session/group found below. Never exported to a renderer/ordinary product API.
static cx_transport_pin *retain(pid_t pid, const uint8_t hash[20]) {
    cx_transport_pin *pin = calloc(1, sizeof(*pin));
    if (!pin) return NULL;
    pin->pid = pid;
    memcpy(pin->hash, hash, 20);
    mach_msg_type_number_t count = TASK_AUDIT_TOKEN_COUNT;
    if (task_name_for_pid(mach_task_self(), pid, &pin->task) != KERN_SUCCESS ||
        task_info(pin->task, TASK_AUDIT_TOKEN, (task_info_t)&pin->token, &count) != KERN_SUCCESS ||
        count != TASK_AUDIT_TOKEN_COUNT || audit_token_to_pid(pin->token) != pid ||
        audit_token_to_euid(pin->token) != geteuid() || !image_matches(pin)) {
        cx_transport_pin_close(pin);
        return NULL;
    }
    return pin;
}

int cx_transport_retain_child(pid_t pid, const uint8_t hash[20], cx_transport_pin **out) {
    *out = NULL;
    struct proc_bsdinfo info;
    if (getpgrp() != getpid() || getsid(0) != getpid() || pid <= 1 ||
        proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, sizeof(info)) != (int)sizeof(info) ||
        info.pbi_ppid != (uint32_t)getpid() || info.pbi_pgid != (uint32_t)getpid()) return 1;
    *out = retain(pid, hash);
    return *out ? 0 : 1;
}

static bool tuple(const struct socket_fdinfo *info, const struct sockaddr_storage *local,
                  const struct sockaddr_storage *remote) {
    if (info->psi.soi_kind != SOCKINFO_TCP || info->psi.soi_type != SOCK_STREAM ||
        info->psi.soi_protocol != IPPROTO_TCP || info->psi.soi_family != local->ss_family ||
        local->ss_family != remote->ss_family || info->psi.soi_proto.pri_tcp.tcpsi_state != TSI_S_ESTABLISHED) return false;
    const struct in_sockinfo *in = &info->psi.soi_proto.pri_tcp.tcpsi_ini;
    if (local->ss_family == AF_INET) {
        const struct sockaddr_in *l = (const struct sockaddr_in *)local, *r = (const struct sockaddr_in *)remote;
        return (in->insi_vflag & INI_IPV4) && (uint16_t)in->insi_lport == r->sin_port &&
            (uint16_t)in->insi_fport == l->sin_port &&
            in->insi_laddr.ina_46.i46a_addr4.s_addr == r->sin_addr.s_addr &&
            in->insi_faddr.ina_46.i46a_addr4.s_addr == l->sin_addr.s_addr;
    }
    if (local->ss_family == AF_INET6) {
        const struct sockaddr_in6 *l = (const struct sockaddr_in6 *)local, *r = (const struct sockaddr_in6 *)remote;
        return (in->insi_vflag & INI_IPV6) && (uint16_t)in->insi_lport == r->sin6_port &&
            (uint16_t)in->insi_fport == l->sin6_port &&
            !memcmp(&in->insi_laddr.ina_6, &r->sin6_addr, sizeof(r->sin6_addr)) &&
            !memcmp(&in->insi_faddr.ina_6, &l->sin6_addr, sizeof(l->sin6_addr));
    }
    return false;
}

// 0 found, 1 no match, 2 unavailable/invalid/bounded table. No application bytes.
static int find_socket(pid_t pid, const struct sockaddr_storage *local,
                       const struct sockaddr_storage *remote) {
    struct proc_fdinfo fds[4096];
    int required = proc_pidinfo(pid, PROC_PIDLISTFDS, 0, NULL, 0);
    if (required <= 0 || required >= (int)sizeof(fds) || required % sizeof(fds[0])) return 2;
    int size = proc_pidinfo(pid, PROC_PIDLISTFDS, 0, fds, sizeof(fds));
    if (size <= 0 || size >= (int)sizeof(fds) || size % sizeof(fds[0])) return 2;
    for (size_t i = 0; i < (size_t)size / sizeof(fds[0]); ++i) {
        if (fds[i].proc_fdtype != PROX_FDTYPE_SOCKET) continue;
        struct socket_fdinfo first = {0}, second = {0};
        if (proc_pidfdinfo(pid, fds[i].proc_fd, PROC_PIDFDSOCKETINFO, &first, sizeof(first)) != (int)sizeof(first)) {
            if (errno == EBADF || errno == ENOENT || errno == ESRCH) continue;
            return 2;
        }
        if (!tuple(&first, local, remote)) continue;
        if (proc_pidfdinfo(pid, fds[i].proc_fd, PROC_PIDFDSOCKETINFO, &second, sizeof(second)) != (int)sizeof(second) ||
            !tuple(&second, local, remote) || first.psi.soi_so != second.psi.soi_so ||
            first.psi.soi_pcb != second.psi.soi_pcb ||
            first.psi.soi_proto.pri_tcp.tcpsi_ini.insi_gencnt != second.psi.soi_proto.pri_tcp.tcpsi_ini.insi_gencnt) return 2;
        return 0;
    }
    return 1;
}

int cx_transport_socket_peer(const cx_transport_pin *root, int fd, cx_transport_pin **out) {
    *out = NULL;
    if (!image_matches(root) || getpgrp() != getpid() || getsid(0) != getpid()) return 3;
    struct sockaddr_storage local = {0}, remote = {0};
    socklen_t l = sizeof(local), r = sizeof(remote);
    if (getsockname(fd, (struct sockaddr *)&local, &l) || getpeername(fd, (struct sockaddr *)&remote, &r)) return 2;
    pid_t members[65];
    int count = proc_listpgrppids(getpid(), members, sizeof(members));
    if (count <= 0 || count >= (int)(sizeof(members) / sizeof(members[0]))) return 2;
    for (int i = 0; i < count; ++i) {
        if (members[i] <= 1 || members[i] == getpid()) continue;
        struct proc_bsdinfo info;
        if (proc_pidinfo(members[i], PROC_PIDTBSDINFO, 0, &info, sizeof(info)) != (int)sizeof(info)) continue;
        if (info.pbi_pgid != (uint32_t)getpid() || info.pbi_status == SZOMB) continue;
        int found = find_socket(members[i], &local, &remote);
        if (found == 2) return 2;
        if (found) continue;
        cx_transport_pin *peer = retain(members[i], root->hash);
        if (!peer || !image_matches(root) ||
            proc_pidinfo(members[i], PROC_PIDTBSDINFO, 0, &info, sizeof(info)) != (int)sizeof(info) ||
            info.pbi_pgid != (uint32_t)getpid()) {
            cx_transport_pin_close(peer);
            return 3;
        }
        *out = peer;
        return 0;
    }
    return 1;
}

// Caller retains the unreaped group leader. Ignore only observed zombies;
// unavailable or truncated membership data is never interpreted as empty.
int cx_transport_group_empty(pid_t group, pid_t except) {
    pid_t members[65];
    errno = 0;
    int count = proc_listpgrppids(group, members, sizeof(members));
    if (count < 0 || (!count && errno) || count >= (int)(sizeof(members) / sizeof(members[0]))) return -1;
    for (int i = 0; i < count; ++i) {
        if (members[i] == except) continue;
        struct proc_bsdinfo info;
        if (proc_pidinfo(members[i], PROC_PIDTBSDINFO, 0, &info, sizeof(info)) != (int)sizeof(info)) return -1;
        if (info.pbi_pgid != (uint32_t)group || info.pbi_status != SZOMB) return 0;
    }
    return 1;
}
