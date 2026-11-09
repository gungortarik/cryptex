// Synthetic SDK/runtime availability probe only. No listener or trust operation.
#include <Security/Security.h>
#include <dlfcn.h>
#include <stdio.h>
#include <xpc/xpc.h>

// Compile/link the real public declarations without exercising authorization.
int cryptex_probe_requirement_declaration(xpc_connection_t connection, const char *requirement) {
    return xpc_connection_set_peer_code_signing_requirement(connection, requirement);
}

OSStatus cryptex_probe_message_declarations(xpc_object_t message, SecRequirementRef requirement) {
    SecCodeRef code = NULL;
    OSStatus status = SecCodeCreateWithXPCMessage(message, kSecCSDefaultFlags, &code);
    if (status == errSecSuccess && code != NULL) {
        status = SecCodeCheckValidity(code, kSecCSDefaultFlags, requirement);
    }
    if (code != NULL) CFRelease(code);
    return status;
}

int main(void) {
    const char *symbols[] = {
        "xpc_connection_set_peer_code_signing_requirement",
        "SecCodeCreateWithXPCMessage",
        "SecCodeCheckValidity"
    };
    int missing = 0;
    for (unsigned int i = 0; i < sizeof(symbols) / sizeof(symbols[0]); ++i) {
        int present = dlsym(RTLD_DEFAULT, symbols[i]) != NULL;
        printf("%s=%s\n", symbols[i], present ? "present" : "missing");
        if (!present) missing = 1;
    }
    return missing ? 2 : 0;
}
