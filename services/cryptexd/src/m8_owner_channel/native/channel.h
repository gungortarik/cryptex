// Private, single-owner macOS adapter. Neither endpoint dispatches domain work.
#ifndef CRYPTEX_M8_NATIVE_CHANNEL_H
#define CRYPTEX_M8_NATIVE_CHANNEL_H

#include <Security/Security.h>
#include <stdint.h>
#include <sys/types.h>

typedef struct cx_channel cx_channel;
enum cx_result {
    CX_OK, CX_ARGUMENT, CX_RESOURCE, CX_RENDEZVOUS, CX_DEADLINE, CX_TRANSPORT,
    CX_LAYOUT, CX_EPOCH, CX_SEQUENCE, CX_INSTANCE, CX_IMAGE, CX_CLOSED
};
#define CX_MAX_BODY (1024u * 1024u + 4096u)
#define CX_EPOCH_BYTES 16

// Desktop registers before spawning. Only the nonsecret epoch crosses argv.
enum cx_result cx_listen(cx_channel **out, uint8_t epoch[CX_EPOCH_BYTES]);
// child is the supervised, unreaped child, never a PID from a message. The
// caller supplies independently trusted, immutable image policy. No product
// policy factory exists yet. Policy is copied before the handshake.
typedef struct {
    const char *requirement;
    const uint8_t *cdhash;
    size_t hash_size;
} cx_policy;
enum cx_result cx_accept(cx_channel *c, pid_t child, const cx_policy *policy);
enum cx_result cx_connect(cx_channel **out, const uint8_t epoch[CX_EPOCH_BYTES],
                          const cx_policy *policy);
// Errors during an exchange invalidate the session. Close also owns cleanup
// for partial handshakes. Never share a handle across threads or fork.
enum cx_result cx_send(cx_channel *c, const uint8_t *body, size_t size);
enum cx_result cx_receive(cx_channel *c, uint8_t *body, size_t capacity, size_t *size);
void cx_close(cx_channel *c);
#endif
