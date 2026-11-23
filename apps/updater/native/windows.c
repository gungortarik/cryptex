#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <wintrust.h>
#include <softpub.h>
#include <string.h>

static int pinned(PCCERT_CONTEXT cert, const unsigned char *der, size_t size) {
    return cert && cert->cbCertEncoded == size && !memcmp(cert->pbCertEncoded, der, size);
}

int cx_update_image(const wchar_t *path, const unsigned char *pin, size_t pin_size) {
    GUID action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    WINTRUST_FILE_INFO file = {0};
    WINTRUST_DATA trust = {0};
    file.cbStruct = sizeof(file);
    file.pcwszFilePath = path;
    trust.cbStruct = sizeof(trust);
    trust.dwUIChoice = WTD_UI_NONE;
    trust.dwUnionChoice = WTD_CHOICE_FILE;
    trust.pFile = &file;
    trust.dwStateAction = WTD_STATEACTION_VERIFY;
    trust.dwProvFlags = WTD_CACHE_ONLY_URL_RETRIEVAL | WTD_DISABLE_MD2_MD4;
    int ok = 0;
    if (WinVerifyTrust(INVALID_HANDLE_VALUE, &action, &trust) == ERROR_SUCCESS) {
        CRYPT_PROVIDER_DATA *data = WTHelperProvDataFromStateData(trust.hWVTStateData);
        CRYPT_PROVIDER_SGNR *signer = data ? WTHelperGetProvSignerFromChain(data, 0, FALSE, 0) : NULL;
        CRYPT_PROVIDER_CERT *leaf = signer ? WTHelperGetProvCertFromChain(signer, 0) : NULL;
        ok = leaf && pinned(leaf->pCert, pin, pin_size);
    }
    trust.dwStateAction = WTD_STATEACTION_CLOSE;
    WinVerifyTrust(INVALID_HANDLE_VALUE, &action, &trust);
    return ok ? 0 : 1;
}

int cx_verify_inventory(const unsigned char *cms, size_t size,
                        const unsigned char *content, size_t length,
                        const unsigned char *pin, size_t pin_size) {
    CRYPT_VERIFY_MESSAGE_PARA params = {0};
    PCCERT_CONTEXT signer = NULL;
    unsigned char decoded[512];
    DWORD decoded_size = sizeof(decoded);
    params.cbSize = sizeof(params);
    params.dwMsgAndCertEncodingType = X509_ASN_ENCODING | PKCS_7_ASN_ENCODING;
    if (size > 65536 || length > sizeof(decoded) ||
        CryptGetMessageSignerCount(params.dwMsgAndCertEncodingType, cms, (DWORD)size) != 1) return 1;
    BOOL verified = CryptVerifyMessageSignature(&params, 0, cms, (DWORD)size,
                                               decoded, &decoded_size, &signer);
    int ok = verified && pinned(signer, pin, pin_size) &&
             decoded_size == length && !memcmp(decoded, content, length);
    if (signer) CertFreeCertificateContext(signer);
    return ok ? 0 : 1;
}
