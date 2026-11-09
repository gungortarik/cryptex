// Synthetic nonce echo only. No Cryptex runtime, files, trust or transport.
#include <Security/Security.h>
#include <dispatch/dispatch.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <xpc/xpc.h>

static dispatch_queue_t queue;
static SecRequirementRef client_requirement;
static xpc_endpoint_t saved_endpoint;
static char saved_nonce[33];
static const char *client_text;
static unsigned record_count;

static int nonce_valid(const char *text) {
    if (!text || strlen(text) != 32) return 0;
    return strspn(text, "0123456789abcdef") == 32;
}

static int message_valid(xpc_object_t message, SecRequirementRef requirement) {
    SecCodeRef code = NULL;
    OSStatus status = SecCodeCreateWithXPCMessage(message, kSecCSDefaultFlags, &code);
    if (status == errSecSuccess && code) {
        status = SecCodeCheckValidity(code, kSecCSStrictValidate, requirement);
    } else if (status == errSecSuccess) {
        status = errSecCSInternalError;
    }
    if (code) CFRelease(code);
    return status == errSecSuccess;
}

static SecRequirementRef parse_requirement(const char *text) {
    CFStringRef string = CFStringCreateWithCString(NULL, text, kCFStringEncodingUTF8);
    SecRequirementRef result = NULL;
    if (!string) return NULL;
    OSStatus status = SecRequirementCreateWithString(string, kSecCSDefaultFlags, &result);
    CFRelease(string);
    return status == errSecSuccess ? result : NULL;
}

static void reply_status(xpc_connection_t peer, xpc_object_t message,
                         const char *status, const char *nonce, xpc_object_t endpoint) {
    xpc_object_t reply = xpc_dictionary_create_reply(message);
    if (!reply) return;
    xpc_dictionary_set_string(reply, "status", status);
    xpc_dictionary_set_string(reply, "nonce", nonce);
    if (endpoint) xpc_dictionary_set_value(reply, "endpoint", endpoint);
    xpc_connection_send_message(peer, reply);
    xpc_release(reply);
}

static void handle_message(xpc_connection_t peer, xpc_object_t message,
                           const char *lane, xpc_connection_t listener) {
    if (xpc_get_type(message) != XPC_TYPE_DICTIONARY) return;
    const char *nonce = xpc_dictionary_get_string(message, "nonce");
    const char *op = xpc_dictionary_get_string(message, "op");
    if (!nonce_valid(nonce) || !op || xpc_dictionary_get_count(message) > 3) return;
    if (++record_count > 128) _exit(126); // Bound the private synthetic log too.
    int valid = message_valid(message, client_requirement);
    // Fixed lane/status plus synthetic nonce only; never dump XPC descriptions.
    printf("{\"lane\":\"%s\",\"nonce\":\"%s\",\"valid\":%s,\"op\":\"%s\"}\n",
           lane, nonce, valid ? "true" : "false",
           strcmp(op, "echo") == 0 ? "echo" : "endpoint");
    fflush(stdout);
    if (!valid) {
        reply_status(peer, message, "message_rejected", nonce, NULL);
        return;
    }
    if (strcmp(op, "echo") == 0) {
        reply_status(peer, message, "echo", nonce, NULL);
    } else if (strcmp(lane, "guard") == 0 && strcmp(op, "get") == 0) {
        xpc_endpoint_t endpoint = xpc_endpoint_create(listener);
        reply_status(peer, message, "endpoint", nonce, endpoint);
        xpc_release(endpoint);
    } else if (strcmp(lane, "relay") == 0 && strcmp(op, "put") == 0) {
        xpc_object_t endpoint = xpc_dictionary_get_value(message, "endpoint");
        if (!endpoint || xpc_get_type(endpoint) != XPC_TYPE_ENDPOINT || saved_endpoint) {
            reply_status(peer, message, "relay_refused", nonce, NULL);
            return;
        }
        saved_endpoint = xpc_retain(endpoint);
        memcpy(saved_nonce, nonce, 33);
        reply_status(peer, message, "stored", nonce, NULL);
    } else if (strcmp(lane, "relay") == 0 && strcmp(op, "take") == 0 &&
               saved_endpoint && strcmp(nonce, saved_nonce) == 0) {
        reply_status(peer, message, "endpoint", nonce, saved_endpoint);
        xpc_release(saved_endpoint);
        saved_endpoint = NULL;
    } else {
        reply_status(peer, message, "operation_refused", nonce, NULL);
    }
}

static void configure_listener(xpc_connection_t listener, const char *lane, int guarded) {
    xpc_connection_set_event_handler(listener, ^(xpc_object_t event) {
        if (xpc_get_type(event) != XPC_TYPE_CONNECTION) return;
        xpc_connection_t peer = event;
        xpc_connection_set_target_queue(peer, queue);
        // The observer deliberately isolates per-message validation for comparison.
        if (guarded && xpc_connection_set_peer_code_signing_requirement(peer, client_text) != 0) {
            xpc_connection_cancel(peer);
            return;
        }
        xpc_connection_set_event_handler(peer, ^(xpc_object_t message) {
            handle_message(peer, message, lane, listener);
        });
        xpc_connection_resume(peer);
    });
    xpc_connection_resume(listener);
}

static xpc_object_t request(xpc_connection_t connection, const char *op,
                            const char *nonce, xpc_object_t endpoint) {
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    __block xpc_object_t response = NULL;
    xpc_object_t message = xpc_dictionary_create(NULL, NULL, 0);
    xpc_dictionary_set_string(message, "op", op);
    xpc_dictionary_set_string(message, "nonce", nonce);
    if (endpoint) xpc_dictionary_set_value(message, "endpoint", endpoint);
    xpc_connection_send_message_with_reply(connection, message, queue, ^(xpc_object_t reply) {
        response = xpc_retain(reply);
        dispatch_semaphore_signal(done);
    });
    xpc_release(message);
    if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) != 0) {
        // Process exits immediately: do not destroy callback state on a timeout.
        puts("{\"result\":\"deadline\"}");
        fflush(stdout);
        _exit(3);
    }
    dispatch_release(done);
    return response;
}

static xpc_connection_t connect_peer(const char *name, const char *requirement, xpc_object_t endpoint) {
    xpc_connection_t connection = endpoint ? xpc_connection_create_from_endpoint(endpoint) :
        xpc_connection_create_mach_service(name, queue, 0);
    if (!connection || xpc_connection_set_peer_code_signing_requirement(connection, requirement) != 0) {
        exit(4);
    }
    xpc_connection_set_event_handler(connection, ^(xpc_object_t event) { (void)event; });
    xpc_connection_resume(connection);
    return connection;
}

static const char *status_of(xpc_object_t response, const char *nonce) {
    if (response == XPC_ERROR_PEER_CODE_SIGNING_REQUIREMENT) return "peer_rejected";
    if (xpc_get_type(response) != XPC_TYPE_DICTIONARY) return "connection_error";
    const char *echo = xpc_dictionary_get_string(response, "nonce");
    const char *status = xpc_dictionary_get_string(response, "status");
    if (!echo || strcmp(echo, nonce) || !status) return "invalid_reply";
    const char *allowed[] = {"echo", "message_rejected", "stored", "endpoint", "relay_refused", "operation_refused"};
    for (unsigned i = 0; i < sizeof(allowed) / sizeof(allowed[0]); ++i) {
        if (strcmp(status, allowed[i]) == 0) return allowed[i];
    }
    return "invalid_reply";
}

static int client(const char *mode, const char *service, const char *requirement, const char *nonce) {
    char name[200];
    const char *lane = strcmp(mode, "observe") == 0 ? "observe" : "guard";
    if (strcmp(mode, "take") == 0) lane = "relay";
    snprintf(name, sizeof(name), "%s.%s", service, lane);
    xpc_connection_t connection = connect_peer(name, requirement, NULL);
    xpc_object_t response = request(connection, strcmp(mode, "put") == 0 ? "get" :
                                    strcmp(mode, "take") == 0 ? "take" : "echo", nonce, NULL);
    const char *status = status_of(response, nonce);
    if ((strcmp(mode, "put") == 0 || strcmp(mode, "take") == 0) && strcmp(status, "endpoint") == 0) {
        xpc_object_t endpoint = xpc_dictionary_get_value(response, "endpoint");
        if (!endpoint || xpc_get_type(endpoint) != XPC_TYPE_ENDPOINT) return 5;
        snprintf(name, sizeof(name), "%s.relay", service);
        xpc_connection_t next = connect_peer(name, requirement, strcmp(mode, "take") == 0 ? endpoint : NULL);
        xpc_object_t result = request(next, strcmp(mode, "put") == 0 ? "put" : "echo", nonce,
                                      strcmp(mode, "put") == 0 ? endpoint : NULL);
        status = status_of(result, nonce);
        printf("{\"result\":\"%s\"}\n", status);
        xpc_release(result);
        xpc_connection_cancel(next);
        xpc_release(next);
    } else {
        printf("{\"result\":\"%s\"}\n", status);
    }
    xpc_release(response);
    xpc_connection_cancel(connection);
    xpc_release(connection);
    return 0;
}

int main(int argc, const char **argv) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    queue = dispatch_queue_create("com.cryptex.m8.synthetic", DISPATCH_QUEUE_SERIAL);
    if (argc == 3 && strcmp(argv[1], "self-smoke") == 0) {
        client_text = argv[2];
        client_requirement = parse_requirement(client_text);
        if (!client_requirement) return 2;
        xpc_connection_t listener = xpc_connection_create(NULL, queue);
        configure_listener(listener, "guard", 1);
        xpc_endpoint_t endpoint = xpc_endpoint_create(listener);
        xpc_connection_t connection = connect_peer(NULL, client_text, endpoint);
        xpc_object_t response = request(connection, "echo", "00000000000000000000000000000000", NULL);
        return strcmp(status_of(response, "00000000000000000000000000000000"), "echo") == 0 ? 0 : 7;
    }
    if (argc == 4 && strcmp(argv[1], "server") == 0) {
        client_text = argv[3];
        client_requirement = parse_requirement(client_text);
        if (!client_requirement || strlen(argv[2]) > 150) return 2;
        const char *lanes[] = {"guard", "observe", "relay"};
        for (unsigned i = 0; i < 3; ++i) {
            char name[200];
            snprintf(name, sizeof(name), "%s.%s", argv[2], lanes[i]);
            xpc_connection_t listener = xpc_connection_create_mach_service(name, queue, XPC_CONNECTION_MACH_SERVICE_LISTENER);
            if (!listener) return 3;
            configure_listener(listener, lanes[i], i != 1);
        }
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 90 * NSEC_PER_SEC), queue, ^{ _exit(124); });
        dispatch_main();
    }
    if (argc == 5 && nonce_valid(argv[4]) && strlen(argv[2]) < 150 &&
        (strcmp(argv[1], "guard") == 0 || strcmp(argv[1], "observe") == 0 ||
         strcmp(argv[1], "put") == 0 || strcmp(argv[1], "take") == 0)) {
        return client(argv[1], argv[2], argv[3], argv[4]);
    }
    return 2;
}
