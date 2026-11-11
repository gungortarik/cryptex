// Native CMS verification against an independently provisioned exact signer.
#include <Security/Security.h>
#include <Security/CMSDecoder.h>
#include <stdint.h>
#include <string.h>

int cx_verify_inventory(const uint8_t *cms, size_t cms_size,
                        const uint8_t *content, size_t content_size,
                        const uint8_t *signer_der, size_t signer_size) {
    if (!cms || !cms_size || cms_size > 2*1024*1024 || !content ||
        !content_size || content_size > 1024*1024 || !signer_der ||
        !signer_size || signer_size > 65536) return 1;
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
    policy = SecPolicyCreateCodeSigning();
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
