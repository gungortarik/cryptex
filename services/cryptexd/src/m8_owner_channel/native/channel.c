#include "channel.h"
#include <bsm/libbsm.h>
#include <mach/mach.h>
#include <servers/bootstrap.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

enum { FRAME_ID = 0x43584d38, HELLO = 1, ACK, DATA, WAIT_MS = 2500 };
typedef struct {
    mach_msg_header_t header;
    uint32_t version, kind;
    uint64_t sequence;
    uint8_t epoch[CX_EPOCH_BYTES];
    uint32_t size;
    uint8_t body[];
} Packet;

struct cx_channel {
    mach_port_t inbox, peer, task;
    pid_t pid;
    audit_token_t sender;
    SecRequirementRef requirement;
    CFDataRef cdhash;
    uint8_t epoch[CX_EPOCH_BYTES];
    uint64_t sent, received;
    bool parent_is_peer, pinned, closed;
};

// One monotonic budget per handshake or data call. Signing APIs are
// synchronous; elapsed time is rechecked before sending or publishing bytes.
uint64_t cx_now_ms(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) return UINT64_MAX;
    return (uint64_t)t.tv_sec * 1000 + (uint64_t)t.tv_nsec / 1000000;
}

static uint64_t deadline_ms(void) {
    uint64_t now = cx_now_ms();
    return now > UINT64_MAX - WAIT_MS ? 0 : now + WAIT_MS;
}

static mach_msg_timeout_t remaining_ms(uint64_t deadline) {
    uint64_t now = cx_now_ms();
    return now < deadline && deadline - now <= UINT32_MAX ? (mach_msg_timeout_t)(deadline - now) : 0;
}

static bool release_send(mach_port_t right) {
    return !MACH_PORT_VALID(right) || mach_port_deallocate(mach_task_self(), right) == KERN_SUCCESS;
}

enum cx_result cx_close(cx_channel *c) {
    if (!c) return CX_OK;
    bool ok = release_send(c->peer);
    if (!release_send(c->task)) ok = false;
    if (MACH_PORT_VALID(c->inbox) &&
        mach_port_mod_refs(mach_task_self(), c->inbox, MACH_PORT_RIGHT_RECEIVE, -1) != KERN_SUCCESS)
        ok = false;
    if (c->requirement) CFRelease(c->requirement);
    if (c->cdhash) CFRelease(c->cdhash);
    free(c);
    return ok ? CX_OK : CX_CLEANUP;
}

static bool live(cx_channel *c) {
    int pid = 0;
    audit_token_t observed;
    mach_msg_type_number_t count = TASK_AUDIT_TOKEN_COUNT;
    return (!c->parent_is_peer || getppid() == c->pid) &&
        pid_for_task(c->task, &pid) == KERN_SUCCESS && pid == c->pid &&
        task_info(c->task, TASK_AUDIT_TOKEN, (task_info_t)&observed, &count) == KERN_SUCCESS &&
        count == TASK_AUDIT_TOKEN_COUNT && !memcmp(&observed, &c->sender, sizeof(observed));
}

static enum cx_result configure(cx_channel *c, pid_t pid, const cx_policy *policy) {
    if (pid <= 1 || !policy || !policy->requirement || !policy->cdhash ||
        !policy->hash_size || policy->hash_size > 64) return CX_ARGUMENT;
    c->pid = pid;
    CFStringRef text = CFStringCreateWithCString(NULL, policy->requirement, kCFStringEncodingUTF8);
    if (!text) return CX_IMAGE;
    OSStatus status = SecRequirementCreateWithString(text, 0, &c->requirement);
    CFRelease(text);
    if (status != errSecSuccess || !c->requirement) return CX_IMAGE;
    c->cdhash = CFDataCreate(NULL, policy->cdhash, (CFIndex)policy->hash_size);
    if (!c->cdhash) return CX_RESOURCE;
    mach_msg_type_number_t count = TASK_AUDIT_TOKEN_COUNT;
    if (task_name_for_pid(mach_task_self(), pid, &c->task) != KERN_SUCCESS ||
        task_info(c->task, TASK_AUDIT_TOKEN, (task_info_t)&c->sender, &count) != KERN_SUCCESS ||
        count != TASK_AUDIT_TOKEN_COUNT || audit_token_to_pid(c->sender) != pid || !live(c))
        return CX_INSTANCE;
    return CX_OK;
}

// Called with a kernel trailer on receive, and with that pinned trailer on
// send. The requirement and CodeDirectory hash come from outside the wire.
static enum cx_result check_sender(cx_channel *c, const audit_token_t *token) {
    if (!live(c) || audit_token_to_pid(*token) != c->pid ||
        memcmp(token, &c->sender, sizeof(*token))) return CX_INSTANCE;
    CFDataRef audit = CFDataCreate(NULL, (const UInt8 *)token, sizeof(*token));
    CFNumberRef pid = CFNumberCreate(NULL, kCFNumberIntType, &c->pid);
    const void *keys[] = {kSecGuestAttributeAudit, kSecGuestAttributePid};
    const void *values[] = {audit, pid};
    CFDictionaryRef attrs = audit && pid ? CFDictionaryCreate(NULL, keys, values, 2,
        &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks) : NULL;
    SecCodeRef code = NULL;
    CFDictionaryRef info = NULL;
    enum cx_result result = CX_IMAGE;
    if (attrs && SecCodeCopyGuestWithAttributes(NULL, attrs, 0, &code) == errSecSuccess && code &&
        SecCodeCheckValidity(code, kSecCSStrictValidate, c->requirement) == errSecSuccess &&
        SecCodeCopySigningInformation((SecStaticCodeRef)code, 0, &info) == errSecSuccess && info) {
        CFTypeRef hash = CFDictionaryGetValue(info, kSecCodeInfoUnique);
        if (hash && CFGetTypeID(hash) == CFDataGetTypeID() && CFEqual(hash, c->cdhash))
            result = live(c) ? CX_OK : CX_INSTANCE;
    }
    if (info) CFRelease(info);
    if (code) CFRelease(code);
    if (attrs) CFRelease(attrs);
    if (pid) CFRelease(pid);
    if (audit) CFRelease(audit);
    return result;
}

static void service_name(const uint8_t epoch[CX_EPOCH_BYTES], char name[BOOTSTRAP_MAX_NAME_LEN]) {
    static const char prefix[] = "com.cryptex.m8.channel.";
    memcpy(name, prefix, sizeof(prefix));
    for (size_t i = 0; i < CX_EPOCH_BYTES; ++i)
        snprintf(name + sizeof(prefix) - 1 + 2*i, 3, "%02x", epoch[i]);
}

static size_t wire_size(size_t size) {
    return (offsetof(Packet, body) + size + 3u) & ~(size_t)3u;
}

static enum cx_result transmit(cx_channel *c, mach_port_t destination, uint32_t kind,
                               const uint8_t *body, size_t size, uint64_t deadline) {
    if (!remaining_ms(deadline)) return CX_DEADLINE;
    size_t bytes = wire_size(size);
    Packet *packet = calloc(1, bytes < sizeof(Packet) ? sizeof(Packet) : bytes);
    if (!packet) return CX_RESOURCE;
    mach_msg_header_t *h = &packet->header;
    // Lend no owned references to mach_msg. Give the syscall dedicated
    // MOVE_SEND references; even a pseudo-receive returns only those refs.
    enum cx_result result = CX_RESOURCE;
    if (mach_port_mod_refs(mach_task_self(), destination, MACH_PORT_RIGHT_SEND, 1) != KERN_SUCCESS)
        goto done;
    h->msgh_remote_port = destination;
    if (kind != DATA) {
        if (mach_port_insert_right(mach_task_self(), c->inbox, c->inbox, MACH_MSG_TYPE_MAKE_SEND)
            != KERN_SUCCESS) goto release;
        h->msgh_local_port = c->inbox;
    }
    h->msgh_bits = MACH_MSGH_BITS(MACH_MSG_TYPE_MOVE_SEND, kind == DATA ? 0 : MACH_MSG_TYPE_MOVE_SEND);
    h->msgh_size = (mach_msg_size_t)bytes;
    h->msgh_id = FRAME_ID;
    packet->version = 1;
    packet->kind = kind;
    packet->sequence = c->sent + 1;
    memcpy(packet->epoch, c->epoch, sizeof(c->epoch));
    packet->size = (uint32_t)size;
    if (size) memcpy(packet->body, body, size);
    mach_msg_timeout_t remaining = remaining_ms(deadline);
    if (!remaining) { result = CX_DEADLINE; goto release; }
    mach_msg_return_t kr = mach_msg(h, MACH_SEND_MSG | MACH_SEND_TIMEOUT | MACH_SEND_INTERRUPT,
        h->msgh_size, 0, MACH_PORT_NULL, remaining, MACH_PORT_NULL);
    if (kr == MACH_MSG_SUCCESS) {
        ++c->sent;
        result = remaining_ms(deadline) ? CX_OK : CX_DEADLINE;
        goto done;
    }
    result = kr == MACH_SEND_TIMED_OUT ? CX_DEADLINE : CX_TRANSPORT;
release:
    if (!release_send(h->msgh_remote_port)) result = CX_CLEANUP;
    if (!release_send(h->msgh_local_port)) result = CX_CLEANUP;
done:
    free(packet);
    return result;
}

// Pure envelope validation. Received rights still belong to the Mach message
// until read_frame validates its kernel sender and transfers the endpoint.
static enum cx_result envelope(const cx_channel *c, const Packet *p, size_t bytes,
                               uint32_t kind, size_t capacity, audit_token_t *sender) {
    if (bytes < wire_size(0) + sizeof(mach_msg_audit_trailer_t)) return CX_LAYOUT;
    const mach_msg_header_t *h = &p->header;
    if (h->msgh_id != FRAME_ID || h->msgh_local_port != c->inbox ||
        (h->msgh_bits & MACH_MSGH_BITS_COMPLEX) || h->msgh_voucher_port != MACH_PORT_NULL ||
        h->msgh_size < offsetof(Packet, body) || p->version != 1 || p->kind != kind ||
        p->size > capacity || h->msgh_size != wire_size(p->size) ||
        h->msgh_size > bytes - sizeof(mach_msg_audit_trailer_t)) return CX_LAYOUT;
    if (kind == DATA) {
        if (!p->size || h->msgh_remote_port != MACH_PORT_NULL || MACH_MSGH_BITS_REMOTE(h->msgh_bits)) return CX_LAYOUT;
    } else if (p->size || !MACH_PORT_VALID(h->msgh_remote_port) ||
               MACH_MSGH_BITS_REMOTE(h->msgh_bits) != MACH_MSG_TYPE_PORT_SEND) return CX_LAYOUT;
    mach_msg_audit_trailer_t trailer;
    memcpy(&trailer, (const uint8_t *)p + h->msgh_size, sizeof(trailer));
    if (trailer.msgh_trailer_type != MACH_MSG_TRAILER_FORMAT_0 ||
        trailer.msgh_trailer_size != sizeof(trailer)) return CX_LAYOUT;
    if (memcmp(c->epoch, p->epoch, sizeof(c->epoch))) return CX_EPOCH;
    if (!p->sequence || p->sequence != c->received + 1) return CX_SEQUENCE;
    *sender = trailer.msgh_audit;
    return CX_OK;
}

static enum cx_result read_frame(cx_channel *c, uint32_t kind, uint8_t *body,
                                 size_t capacity, size_t *size, uint64_t deadline) {
    if (!remaining_ms(deadline)) return CX_DEADLINE;
    const size_t bytes = wire_size(capacity) + sizeof(mach_msg_max_trailer_t);
    Packet *p = calloc(1, bytes);
    if (!p) return CX_RESOURCE;
    mach_msg_header_t *h = &p->header;
    mach_msg_option_t options = MACH_RCV_MSG | MACH_RCV_TIMEOUT | MACH_RCV_INTERRUPT |
        MACH_RCV_TRAILER_TYPE(MACH_MSG_TRAILER_FORMAT_0) | MACH_RCV_TRAILER_ELEMENTS(MACH_RCV_TRAILER_AUDIT);
    mach_msg_timeout_t remaining = remaining_ms(deadline);
    if (!remaining) { free(p); return CX_DEADLINE; }
    mach_msg_return_t kr = mach_msg(h, options, 0, (mach_msg_size_t)bytes, c->inbox, remaining, MACH_PORT_NULL);
    enum cx_result result = CX_LAYOUT;
    if (kr != MACH_MSG_SUCCESS) {
        if ((kr & ~MACH_MSG_MASK) == MACH_RCV_BODY_ERROR) mach_msg_destroy(h);
        free(p);
        return kr == MACH_RCV_TIMED_OUT ? CX_DEADLINE : CX_TRANSPORT;
    }
    audit_token_t sender;
    result = envelope(c, p, bytes, kind, capacity, &sender);
    if (result != CX_OK) goto done;
    if (!remaining_ms(deadline)) { result = CX_DEADLINE; goto done; }
    result = check_sender(c, &sender);
    if (result != CX_OK) goto done;
    if (!remaining_ms(deadline)) { result = CX_DEADLINE; goto done; }
    if (kind == DATA) {
        memcpy(body, p->body, p->size);
        *size = p->size;
    } else {
        c->pinned = true;
        c->peer = h->msgh_remote_port;
        h->msgh_remote_port = MACH_PORT_NULL;
    }
    ++c->received;
done:
    mach_msg_destroy(h);
    free(p);
    return result;
}

enum cx_result cx_listen(cx_channel **out, uint8_t epoch[CX_EPOCH_BYTES]) {
    if (!out || !epoch) return CX_ARGUMENT;
    *out = NULL;
    cx_channel *c = calloc(1, sizeof(*c));
    if (!c) return CX_RESOURCE;
    arc4random_buf(c->epoch, sizeof(c->epoch));
    char name[BOOTSTRAP_MAX_NAME_LEN];
    service_name(c->epoch, name);
    if (bootstrap_check_in(bootstrap_port, name, &c->inbox) != BOOTSTRAP_SUCCESS) {
        return cx_close(c) == CX_OK ? CX_RENDEZVOUS : CX_CLEANUP;
    }
    memcpy(epoch, c->epoch, sizeof(c->epoch));
    *out = c;
    return CX_OK;
}

enum cx_result cx_accept(cx_channel *c, pid_t child, const cx_policy *policy) {
    if (!c || c->closed || c->requirement || c->pinned) return CX_CLOSED;
    uint64_t deadline = deadline_ms();
    enum cx_result result = configure(c, child, policy);
    if (result == CX_OK) result = read_frame(c, HELLO, NULL, 0, NULL, deadline);
    if (result == CX_OK) {
        // The named inbox only receives the public HELLO. Hand the pinned
        // child a fresh, unregistered endpoint in the sender-attributed ACK.
        if (mach_port_mod_refs(mach_task_self(), c->inbox, MACH_PORT_RIGHT_RECEIVE, -1)
            != KERN_SUCCESS) result = CX_RESOURCE;
        else {
            c->inbox = MACH_PORT_NULL;
            if (mach_port_allocate(mach_task_self(), MACH_PORT_RIGHT_RECEIVE, &c->inbox)
                != KERN_SUCCESS) result = CX_RESOURCE;
        }
    }
    if (result == CX_OK) result = transmit(c, c->peer, ACK, NULL, 0, deadline);
    c->closed = result != CX_OK;
    return result;
}

enum cx_result cx_connect(cx_channel **out, const uint8_t epoch[CX_EPOCH_BYTES],
                          const cx_policy *policy) {
    if (!out || !epoch) return CX_ARGUMENT;
    *out = NULL;
    cx_channel *c = calloc(1, sizeof(*c));
    if (!c) return CX_RESOURCE;
    memcpy(c->epoch, epoch, sizeof(c->epoch));
    c->parent_is_peer = true;
    uint64_t deadline = deadline_ms();
    enum cx_result result = configure(c, getppid(), policy);
    mach_port_t service = MACH_PORT_NULL;
    char name[BOOTSTRAP_MAX_NAME_LEN];
    service_name(epoch, name);
    if (result == CX_OK && bootstrap_look_up(bootstrap_port, name, &service) != BOOTSTRAP_SUCCESS)
        result = CX_RENDEZVOUS;
    if (result == CX_OK && mach_port_allocate(mach_task_self(), MACH_PORT_RIGHT_RECEIVE, &c->inbox)
        != KERN_SUCCESS) result = CX_RESOURCE;
    // Only the public HELLO goes to the lookup result. The ACK must carry the
    // actual parent's send right before any application bytes can be sent.
    if (result == CX_OK) result = transmit(c, service, HELLO, NULL, 0, deadline);
    if (!release_send(service)) result = CX_CLEANUP;
    if (result == CX_OK) result = read_frame(c, ACK, NULL, 0, NULL, deadline);
    if (result != CX_OK) return cx_close(c) == CX_OK ? result : CX_CLEANUP;
    *out = c;
    return CX_OK;
}

enum cx_result cx_send(cx_channel *c, const uint8_t *body, size_t size, uint64_t deadline) {
    if (!c || c->closed || !c->pinned) return CX_CLOSED;
    enum cx_result result = !remaining_ms(deadline) ? CX_DEADLINE :
        !body || !size || size > CX_MAX_BODY ? CX_ARGUMENT :
        c->sent == UINT64_MAX ? CX_SEQUENCE : check_sender(c, &c->sender);
    if (result == CX_OK) result = transmit(c, c->peer, DATA, body, size, deadline);
    c->closed = result != CX_OK;
    return result;
}

enum cx_result cx_receive(cx_channel *c, uint8_t *body, size_t capacity, size_t *size, uint64_t deadline) {
    if (size) *size = 0;
    if (!c || c->closed || !c->pinned) return CX_CLOSED;
    enum cx_result result = !body || !capacity || capacity > CX_MAX_BODY || !size ? CX_ARGUMENT :
        read_frame(c, DATA, body, capacity, size, deadline);
    c->closed = result != CX_OK;
    return result;
}
