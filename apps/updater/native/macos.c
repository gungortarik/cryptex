#include <Security/Security.h>
#include <stdint.h>
#include <string.h>

int cx_update_image(const char *path, const char *identifier, const uint8_t *pin, size_t pin_size) {
    CFURLRef url = CFURLCreateFromFileSystemRepresentation(NULL, (const uint8_t *)path,
                                                           (CFIndex)strlen(path), false);
    SecStaticCodeRef code = NULL;
    CFDictionaryRef info = NULL;
    CFDataRef actual = NULL;
    CFStringRef role = CFStringCreateWithCString(NULL, identifier, kCFStringEncodingUTF8);
    int ok = 0;
    if (!url || !role || SecStaticCodeCreateWithPath(url, kSecCSDefaultFlags, &code) || !code ||
        SecStaticCodeCheckValidity(code, kSecCSStrictValidate | kSecCSCheckAllArchitectures, NULL) ||
        SecCodeCopySigningInformation(code, kSecCSSigningInformation, &info) || !info) goto done;
    CFStringRef id = CFDictionaryGetValue(info, kSecCodeInfoIdentifier);
    CFArrayRef certs = CFDictionaryGetValue(info, kSecCodeInfoCertificates);
    if (!id || !CFEqual(id, role) || !certs || CFArrayGetCount(certs) == 0) goto done;
    actual = SecCertificateCopyData((SecCertificateRef)CFArrayGetValueAtIndex(certs, 0));
    ok = actual && (size_t)CFDataGetLength(actual) == pin_size &&
         !memcmp(CFDataGetBytePtr(actual), pin, pin_size);
done:
    if (actual) CFRelease(actual);
    if (info) CFRelease(info);
    if (code) CFRelease(code);
    if (url) CFRelease(url);
    if (role) CFRelease(role);
    return ok ? 0 : 1;
}
