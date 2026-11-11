// Native unit coverage for the private decoder and Mach ownership rules.
// Build separately so test controls are never part of the daemon archive.
#include "../src/m8_owner_channel/native/channel.c"
#include <assert.h>

static void check_envelopes(void) {
    cx_channel c = {.inbox = 1, .received = 1};
    const size_t bytes = wire_size(1) + sizeof(mach_msg_audit_trailer_t);
    Packet *p = calloc(1, bytes);
    assert(p);
    p->header.msgh_id = FRAME_ID;
    p->header.msgh_local_port = c.inbox;
    p->header.msgh_size = (mach_msg_size_t)wire_size(1);
    p->version = 1;
    p->kind = DATA;
    p->size = 1;
    p->sequence = 2;
    mach_msg_audit_trailer_t trailer = {.msgh_trailer_type = MACH_MSG_TRAILER_FORMAT_0,
                                        .msgh_trailer_size = sizeof(trailer)};
    memcpy((uint8_t *)p + p->header.msgh_size, &trailer, sizeof(trailer));
    audit_token_t sender;
    assert(envelope(&c, p, bytes, DATA, 1, &sender) == CX_OK);
    assert(envelope(&c, p, bytes - 1, DATA, 1, &sender) == CX_LAYOUT);
    assert(envelope(&c, p, bytes, DATA, 0, &sender) == CX_LAYOUT);
    p->sequence = 1;
    assert(envelope(&c, p, bytes, DATA, 1, &sender) == CX_SEQUENCE);
    p->sequence = 3;
    assert(envelope(&c, p, bytes, DATA, 1, &sender) == CX_SEQUENCE);
    p->sequence = 2;
    p->epoch[0] = 1;
    assert(envelope(&c, p, bytes, DATA, 1, &sender) == CX_EPOCH);
    p->epoch[0] = 0;
    p->header.msgh_bits |= MACH_MSGH_BITS_COMPLEX;
    assert(envelope(&c, p, bytes, DATA, 1, &sender) == CX_LAYOUT);
    free(p);
}

static void check_name_removal(void) {
    cx_channel *listener = NULL;
    uint8_t epoch[CX_EPOCH_BYTES];
    assert(cx_listen(&listener, epoch) == CX_OK);
    char name[BOOTSTRAP_MAX_NAME_LEN];
    service_name(epoch, name);
    mach_port_t send = MACH_PORT_NULL;
    assert(bootstrap_look_up(bootstrap_port, name, &send) == BOOTSTRAP_SUCCESS);
    assert(release_send(send));
    assert(cx_close(listener) == CX_OK);
    uint64_t until = deadline_ms();
    do {
        send = MACH_PORT_NULL;
        kern_return_t kr = bootstrap_look_up(bootstrap_port, name, &send);
        if (kr == BOOTSTRAP_UNKNOWN_SERVICE) return;
        assert(kr == BOOTSTRAP_SUCCESS && release_send(send));
        usleep(10000);
    } while (remaining_ms(until));
    assert(0 && "closed listener retained a registered name");
}

static void check_send_ownership(void) {
    mach_port_t queue = MACH_PORT_NULL;
    assert(mach_port_allocate(mach_task_self(), MACH_PORT_RIGHT_RECEIVE, &queue) == KERN_SUCCESS);
    assert(mach_port_insert_right(mach_task_self(), queue, queue, MACH_MSG_TYPE_MAKE_SEND) == KERN_SUCCESS);
    mach_port_limits_t limits = {.mpl_qlimit = 1};
    assert(mach_port_set_attributes(mach_task_self(), queue, MACH_PORT_LIMITS_INFO,
        (mach_port_info_t)&limits, MACH_PORT_LIMITS_INFO_COUNT) == KERN_SUCCESS);
    cx_channel *c = calloc(1, sizeof(*c));
    assert(c && mach_port_allocate(mach_task_self(), MACH_PORT_RIGHT_RECEIVE, &c->inbox) == KERN_SUCCESS);
    assert(transmit(c, queue, DATA, (const uint8_t *)"a", 1, deadline_ms()) == CX_OK);
    // A failed handshake send holds dedicated references to *both* endpoints.
    assert(transmit(c, queue, ACK, NULL, 0, cx_now_ms() + 50) == CX_DEADLINE);
    mach_port_urefs_t refs = 0;
    assert(mach_port_get_refs(mach_task_self(), queue, MACH_PORT_RIGHT_SEND, &refs) == KERN_SUCCESS && refs == 1);
    assert(mach_port_get_refs(mach_task_self(), c->inbox, MACH_PORT_RIGHT_SEND, &refs) == KERN_SUCCESS && refs == 0);
    assert(cx_close(c) == CX_OK);
    assert(release_send(queue));
    assert(mach_port_mod_refs(mach_task_self(), queue, MACH_PORT_RIGHT_RECEIVE, -1) == KERN_SUCCESS);
}

int main(void) {
    check_envelopes();
    check_name_removal();
    check_send_ownership();
    puts("native envelope, namespace and send-right ownership checks passed");
    return 0;
}
