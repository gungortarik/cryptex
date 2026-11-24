// Keep the OpenSSL private-key stream in memory and import it into this user's Keychain.
#import <Foundation/Foundation.h>
#import <Security/Security.h>
#include <sys/file.h>
#include <fcntl.h>
#include <pwd.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static int enroll(const char *derPath) {
    NSData *der = [NSData dataWithContentsOfFile:[NSString stringWithUTF8String:derPath]];
    NSData *pem = [[NSFileHandle fileHandleWithStandardInput] readDataToEndOfFile];
    if (!der.length || der.length > 65536 || !pem.length || pem.length > 32768) return 2;
    SecCertificateRef cert = SecCertificateCreateWithData(NULL, (__bridge CFDataRef)der);
    if (!cert) return 2;
    SecKeychainRef kc = NULL;
    SecAccessRef access = NULL;
    SecTrustedApplicationRef codesign = NULL;
    CFArrayRef items = NULL;
    SecIdentityRef identity = NULL;
    BOOL addedCert = NO;
    OSStatus err = SecKeychainCopyDefault(&kc);
    if (!err) err = SecTrustedApplicationCreateFromPath("/usr/bin/codesign", &codesign);
    if (!err) err = SecAccessCreate(CFSTR("Cryptex M17 Mac trial signing key"),
                                    (__bridge CFArrayRef)@[(__bridge id)codesign], &access);
    SecKeyImportExportParameters params = {0};
    params.version = SEC_KEY_IMPORT_EXPORT_PARAMS_VERSION;
    params.keyAttributes = CSSM_KEYATTR_PERMANENT | CSSM_KEYATTR_SENSITIVE;
    params.keyUsage = CSSM_KEYUSE_SIGN;
    params.accessRef = access;
    SecExternalFormat format = kSecFormatUnknown;
    SecExternalItemType item = kSecItemTypePrivateKey;
    if (!err) err = SecKeychainItemImport((__bridge CFDataRef)pem, CFSTR("owner.pem"), &format, &item,
                                         0, &params, kc, &items);
    if (!err && (!items || CFArrayGetCount(items) != 1)) err = errSecInvalidItemRef;
    if (!err) {
        SecKeyRef key = (SecKeyRef)CFArrayGetValueAtIndex(items, 0);
        const CSSM_KEY *attributes = NULL;
        if (CFGetTypeID(key) != SecKeyGetTypeID()) err = errSecInvalidItemRef;
        if (!err) err = SecKeyGetCSSMKey(key, &attributes);
        if (!err && (!attributes || attributes->KeyHeader.KeyAttr & CSSM_KEYATTR_EXTRACTABLE)) err = errSecAuthFailed;
    }
    if (!err) { err = SecCertificateAddToKeychain(cert, kc); addedCert = !err; }
    if (!err) err = SecIdentityCreateWithCertificate(kc, cert, &identity);
    if (err) {
        if (addedCert) SecKeychainItemDelete((SecKeychainItemRef)cert);
        if (items) for (CFIndex i = 0; i < CFArrayGetCount(items); i++)
            SecKeychainItemDelete((SecKeychainItemRef)CFArrayGetValueAtIndex(items, i));
        fprintf(stderr, "Keychain enrollment: OSStatus %d\n", (int)err);
    }
    if (identity) CFRelease(identity);
    if (items) CFRelease(items);
    if (access) CFRelease(access);
    if (codesign) CFRelease(codesign);
    if (kc) CFRelease(kc);
    CFRelease(cert);
    return err ? 1 : 0;
}

int main(int argc, char **argv) {
    @autoreleasepool {
        struct passwd *user = getpwuid(getuid());
        if (!user || strcmp(user->pw_name, "cryptexm14") || !getenv("HOME") ||
            strcmp(user->pw_dir, getenv("HOME")) || getuid() == 0 || getuid() != geteuid()) return 2;
        if (argc == 3 && !strcmp(argv[1], "enroll")) return enroll(argv[2]);
        if (argc == 4 && !strcmp(argv[1], "publish")) {
            if (renamex_np(argv[2], argv[3], RENAME_EXCL)) { perror("publish"); return 1; }
            return 0;
        }
        if (argc == 4 && !strcmp(argv[1], "lock")) {
            int fd = open(argv[2], O_RDWR | O_CREAT | O_NOFOLLOW, 0600);
            if (fd < 0 || flock(fd, LOCK_EX | LOCK_NB)) { perror("trial lock"); return 1; }
            execl("/bin/bash", "bash", argv[3], (char *)NULL);
            perror("trial launch");
        }
        return 2;
    }
}
