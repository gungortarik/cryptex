// Owner-local CMS signing with an existing Keychain identity. Never exports a
// private key or changes Keychain ACLs. Apple Security CMSEncoder API:
// https://developer.apple.com/documentation/security/cryptographic-message-syntax-services
#include <Security/Security.h>
#include <Security/CMSEncoder.h>
#include <CommonCrypto/CommonDigest.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <spawn.h>
#include <sys/wait.h>
#include <errno.h>
extern int cx_verify_inventory(const uint8_t *, size_t, const uint8_t *, size_t, const uint8_t *, size_t);

// Apple's streaming encoder emits BER. Re-serialize only public CMS bytes with
// the system CMS implementation, then apply the unchanged product verifier.
// No key is passed to OpenSSL. The enclosing bounded runner owns this process
// group/deadline. Anonymous scratch descriptors disappear even on termination.
static int scratch(void) {
    char name[] = "/tmp/cryptex-owner-cms-XXXXXX";
    int fd = mkstemp(name);
    if (fd < 0) return -1;
    if (unlink(name) || fcntl(fd, F_SETFD, FD_CLOEXEC)) { close(fd); return -1; }
    return fd;
}
static CFDataRef definite_cms(CFDataRef source) {
    int input = -1, output = -1, actions_ready = 0;
    posix_spawn_file_actions_t actions;
    CFDataRef result = NULL;
    unsigned char *bytes = NULL;
    input = scratch(); output = scratch();
    if (input < 0 || output < 0) goto done;
    size_t size = (size_t)CFDataGetLength(source), offset = 0;
    while (offset < size) {
        ssize_t n = write(input, CFDataGetBytePtr(source) + offset, size - offset);
        if (n <= 0) goto done;
        offset += (size_t)n;
    }
    if (lseek(input, 0, SEEK_SET) != 0 || posix_spawn_file_actions_init(&actions)) goto done;
    actions_ready = 1;
    if (posix_spawn_file_actions_adddup2(&actions, input, STDIN_FILENO) ||
        posix_spawn_file_actions_adddup2(&actions, output, STDOUT_FILENO) ||
        posix_spawn_file_actions_addopen(&actions, STDERR_FILENO, "/dev/null", O_WRONLY, 0)) goto done;
    char *args[] = {"/usr/bin/openssl", "cms", "-cmsout", "-inform", "DER", "-outform", "DER", NULL};
    char *env[] = {"PATH=/usr/bin:/bin", "OPENSSL_CONF=/dev/null", NULL};
    pid_t child;
    if (posix_spawn(&child, args[0], &actions, NULL, args, env)) goto done;
    int status;
    pid_t waited;
    do { waited = waitpid(child, &status, 0); } while (waited < 0 && errno == EINTR);
    if (waited != child || !WIFEXITED(status) || WEXITSTATUS(status)) goto done;
    struct stat st;
    if (fstat(output, &st) || st.st_size < 1 || st.st_size > 2097152 || lseek(output, 0, SEEK_SET) != 0) goto done;
    size = (size_t)st.st_size; offset = 0;
    bytes = malloc(size);
    if (!bytes) goto done;
    while (offset < size) {
        ssize_t n = read(output, bytes + offset, size - offset);
        if (n <= 0) goto done;
        offset += (size_t)n;
    }
    result = CFDataCreate(NULL, bytes, (CFIndex)size);
done:
    free(bytes);
    if (actions_ready) posix_spawn_file_actions_destroy(&actions);
    if (input >= 0) close(input);
    if (output >= 0) close(output);
    return result;
}

static int digest_matches(CFDataRef der, const char *wanted) {
    unsigned char digest[CC_SHA256_DIGEST_LENGTH];
    char text[65];
    if (!der || CFDataGetLength(der) <= 0 || CFDataGetLength(der) > 65536) return 0;
    CC_SHA256(CFDataGetBytePtr(der), (CC_LONG)CFDataGetLength(der), digest);
    for (int i = 0; i < 32; i++) snprintf(text + 2*i, 3, "%02x", digest[i]);
    return strcmp(text, wanted) == 0;
}
int main(int argc, char **argv) {
    int result = 2, input = -1, output = -1;
    const char *stage = "arguments";
    OSStatus status = 0;
    int verify_code = 0;
    CFTypeRef found = NULL;
    SecIdentityRef signer = NULL;
    CMSEncoderRef encoder = NULL;
    CFDataRef encoded = NULL;
    CFDataRef selected_der = NULL;
    unsigned char *bytes = NULL;
    CFDictionaryRef query = NULL;
    if (argc != 4 || strlen(argv[1]) != 64) goto done;
    for (int i=0; i<64; i++) if (!strchr("0123456789abcdef", argv[1][i])) goto done;
    const void *keys[] = {kSecClass, kSecReturnRef, kSecMatchLimit};
    const void *values[] = {kSecClassIdentity, kCFBooleanTrue, kSecMatchLimitAll};
    query = CFDictionaryCreate(NULL, keys, values, 3, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    stage = "identity_query";
    if (!query) goto done;
    status = SecItemCopyMatching(query, &found);
    if (status != errSecSuccess || !found || CFGetTypeID(found) != CFArrayGetTypeID()) goto done;
    CFArrayRef identities = (CFArrayRef)found;
    if (CFArrayGetCount(identities) > 1024) goto done;
    for (CFIndex i=0; i<CFArrayGetCount(identities); i++) {
        SecIdentityRef candidate = (SecIdentityRef)CFArrayGetValueAtIndex(identities, i);
        SecCertificateRef cert = NULL;
        if (CFGetTypeID(candidate) != SecIdentityGetTypeID() || SecIdentityCopyCertificate(candidate, &cert) != errSecSuccess) goto done;
        CFDataRef der = SecCertificateCopyData(cert);
        int match = digest_matches(der, argv[1]);
        if (match && !selected_der && der) selected_der = CFRetain(der);
        if (der) CFRelease(der);
        CFRelease(cert);
        if (match) {
            if (signer) goto done; // ambiguous matching entries refuse
            signer = candidate;
        }
    }
    stage = "identity_select";
    if (!signer) goto done;
    stage = "input";
    input = open(argv[2], O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK);
    struct stat st;
    if (input < 0 || fstat(input, &st) || !S_ISREG(st.st_mode) || st.st_nlink != 1 || st.st_size < 1 || st.st_size > 1048576) goto done;
    bytes = malloc((size_t)st.st_size);
    if (!bytes) goto done;
    size_t offset = 0;
    while (offset < (size_t)st.st_size) {
        ssize_t n = read(input, bytes + offset, (size_t)st.st_size - offset);
        if (n <= 0) goto done;
        offset += (size_t)n;
    }
    unsigned char extra;
    if (read(input, &extra, 1) != 0) goto done;
#define CMS_CALL(label, call) do { stage = label; status = (call); if (status != errSecSuccess) goto done; } while (0)
    CMS_CALL("encoder_create", CMSEncoderCreate(&encoder));
    CMS_CALL("encoder_signer", CMSEncoderAddSigners(encoder, signer));
    CMS_CALL("encoder_algorithm", CMSEncoderSetSignerAlgorithm(encoder, kCMSEncoderDigestAlgorithmSHA256));
    CMS_CALL("encoder_content", CMSEncoderSetHasDetachedContent(encoder, false));
    CMS_CALL("encoder_chain", CMSEncoderSetCertificateChainMode(encoder, kCMSCertificateSignerOnly));
    CMS_CALL("encoder_update", CMSEncoderUpdateContent(encoder, bytes, offset));
    CMS_CALL("encoder_finish", CMSEncoderCopyEncodedContent(encoder, &encoded));
#undef CMS_CALL
    if (!encoded || CFDataGetLength(encoded) > 2097152) goto done;
    stage = "normalize_der";
    CFDataRef normalized = definite_cms(encoded);
    if (!normalized) goto done;
    CFRelease(encoded);
    encoded = normalized;
    stage = "verify_inventory";
    verify_code = cx_verify_inventory(CFDataGetBytePtr(encoded), (size_t)CFDataGetLength(encoded), bytes, offset,
                                     CFDataGetBytePtr(selected_der), (size_t)CFDataGetLength(selected_der));
    if (verify_code) goto done;
    stage = "output";
    output = open(argv[3], O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0600);
    if (output < 0) goto done;
    offset = 0;
    while (offset < (size_t)CFDataGetLength(encoded)) {
        ssize_t n = write(output, CFDataGetBytePtr(encoded) + offset, (size_t)CFDataGetLength(encoded) - offset);
        if (n <= 0) goto done;
        offset += (size_t)n;
    }
    if (fsync(output)) goto done;
    result = 0;
done:
    if (input >= 0) close(input);
    if (output >= 0 && close(output)) result = 2;
    if (encoded) CFRelease(encoded);
    if (selected_der) CFRelease(selected_der);
    if (encoder) CFRelease(encoder);
    if (found) CFRelease(found);
    if (query) CFRelease(query);
    free(bytes);
    if (result) fprintf(stderr, "owner_cms_signing_refused stage=%s status=%d verify=%d\n", stage, (int)status, verify_code);
    return result;
}
