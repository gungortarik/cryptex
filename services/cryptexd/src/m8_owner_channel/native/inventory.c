// Native CMS verification against an independently provisioned exact signer.
#include <Security/Security.h>
#include <Security/CMSDecoder.h>
#include <stdint.h>
#include <string.h>

// Admit only DER ContentInfo(signedData) containing ordinary data, before the
// decoder can encounter any encrypted wrapper or consult decryption identities.
// This is a bounded envelope filter; Security performs signature verification.
typedef struct { const uint8_t *bytes; size_t size; } Span;
static int take(Span *in, uint8_t tag, Span *value) {
    if (in->size < 2 || in->bytes[0] != tag) return 0;
    size_t offset = 2, length = in->bytes[1];
    if (length & 0x80) {
        size_t count = length & 0x7f;
        if (!count || count > sizeof(size_t) || count > in->size - 2 || !in->bytes[2]) return 0;
        length = 0;
        for (size_t i=0; i<count; ++i) {
            if (length > (SIZE_MAX >> 8)) return 0;
            length = (length << 8) | in->bytes[2+i];
        }
        if (length < 128) return 0;
        offset += count;
    }
    if (length > in->size - offset) return 0;
    *value = (Span){in->bytes + offset, length};
    in->bytes += offset + length;
    in->size -= offset + length;
    return 1;
}
static int plain_signed_data(const uint8_t *bytes, size_t size) {
    static const uint8_t signed_oid[] = {0x2a,0x86,0x48,0x86,0xf7,0x0d,1,7,2};
    static const uint8_t data_oid[] = {0x2a,0x86,0x48,0x86,0xf7,0x0d,1,7,1};
    Span input = {bytes,size}, info, oid, explicit_data, signed_data, field, encapsulated;
    if (!take(&input,0x30,&info) || input.size || !take(&info,0x06,&oid) ||
        oid.size != sizeof(signed_oid) || memcmp(oid.bytes,signed_oid,sizeof(signed_oid)) ||
        !take(&info,0xa0,&explicit_data) || info.size ||
        !take(&explicit_data,0x30,&signed_data) || explicit_data.size ||
        !take(&signed_data,0x02,&field) || !take(&signed_data,0x31,&field) ||
        !take(&signed_data,0x30,&encapsulated) || !take(&encapsulated,0x06,&oid) ||
        oid.size != sizeof(data_oid) || memcmp(oid.bytes,data_oid,sizeof(data_oid))) return 0;
    return 1;
}

int cx_verify_inventory(const uint8_t *cms, size_t cms_size,
                        const uint8_t *content, size_t content_size,
                        const uint8_t *signer_der, size_t signer_size) {
    if (!cms || !cms_size || cms_size > 2*1024*1024 || !content ||
        !content_size || content_size > 1024*1024 || !signer_der ||
        !signer_size || signer_size > 65536 || !plain_signed_data(cms,cms_size)) return 1;
    CMSDecoderRef decoder = NULL;
    CFDataRef decoded = NULL, expected = NULL, pinned_data = NULL, actual_data = NULL;
    SecCertificateRef pinned = NULL, actual = NULL;
    SecPolicyRef policy = NULL;
    SecTrustRef trust = NULL;
    CFArrayRef anchors = NULL;
    CFErrorRef error = NULL;
    int result = 2;
    size_t signers = 0;
    CMSSignerStatus status = kCMSSignerUnsigned;
    expected = CFDataCreate(NULL, content, (CFIndex)content_size);
    pinned_data = CFDataCreate(NULL, signer_der, (CFIndex)signer_size);
    if (!expected || !pinned_data) goto done;
    pinned = SecCertificateCreateWithData(NULL, pinned_data);
    policy = SecPolicyCreateWithProperties(kSecPolicyAppleCodeSigning, NULL);
    if (!pinned || !policy || CMSDecoderCreate(&decoder) != errSecSuccess || !decoder ||
        CMSDecoderUpdateMessage(decoder, cms, cms_size) != errSecSuccess ||
        CMSDecoderFinalizeMessage(decoder) != errSecSuccess ||
        CMSDecoderGetNumSigners(decoder, &signers) != errSecSuccess || signers != 1 ||
        CMSDecoderCopyContent(decoder, &decoded) != errSecSuccess || !decoded ||
        !CFEqual(expected, decoded) ||
        CMSDecoderCopySignerCert(decoder, 0, &actual) != errSecSuccess || !actual) goto done;
    actual_data = SecCertificateCopyData(actual);
    if (!actual_data || !CFEqual(actual_data, pinned_data)) goto done;
    // The exact pinned certificate is the explicit trust anchor. Never import
    // it or infer release authority from the user's Keychain trust settings.
    if (CMSDecoderCopySignerStatus(decoder, 0, policy, false, &status, &trust, NULL)
        != errSecSuccess || status != kCMSSignerValid || !trust) goto done;
    const void *values[] = {pinned};
    anchors = CFArrayCreate(NULL, values, 1, &kCFTypeArrayCallBacks);
    if (!anchors || SecTrustSetAnchorCertificates(trust, anchors) != errSecSuccess ||
        SecTrustSetAnchorCertificatesOnly(trust, true) != errSecSuccess ||
        SecTrustSetNetworkFetchAllowed(trust, false) != errSecSuccess ||
        !SecTrustEvaluateWithError(trust, &error)) goto done;
    result = 0;
done:
    if (error) CFRelease(error);
    if (anchors) CFRelease(anchors);
    if (trust) CFRelease(trust);
    if (policy) CFRelease(policy);
    if (actual) CFRelease(actual);
    if (pinned) CFRelease(pinned);
    if (actual_data) CFRelease(actual_data);
    if (pinned_data) CFRelease(pinned_data);
    if (expected) CFRelease(expected);
    if (decoded) CFRelease(decoded);
    if (decoder) CFRelease(decoder);
    return result;
}
