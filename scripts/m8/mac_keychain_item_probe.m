// CI-only metadata inspection. Never requests credential data or changes access.
#import <Foundation/Foundation.h>
#import <Security/Security.h>

static NSString *imageHash(const char *path) {
    NSURL *url = [NSURL fileURLWithPath:[NSString stringWithUTF8String:path]];
    SecStaticCodeRef code = NULL;
    CFDictionaryRef info = NULL;
    if (SecStaticCodeCreateWithPath((__bridge CFURLRef)url, kSecCSDefaultFlags, &code) != errSecSuccess) return nil;
    if (SecCodeCopySigningInformation(code, kSecCSSigningInformation, &info) != errSecSuccess) return nil;
    NSData *hash = ((__bridge NSDictionary *)info)[(__bridge id)kSecCodeInfoUnique];
    if (![hash isKindOfClass:[NSData class]] || hash.length < 20 || hash.length > 32) return nil;
    NSMutableString *text = [NSMutableString string];
    const uint8_t *bytes = hash.bytes;
    for (NSUInteger i = 0; i < hash.length; i++) [text appendFormat:@"%02x", bytes[i]];
    return text;
}

static NSArray *partitions(CFStringRef description) {
    if (!description) return nil;
    NSString *text = (__bridge NSString *)description;
    if (text.length > 65536) return nil;
    NSData *data = [text dataUsingEncoding:NSUTF8StringEncoding];
    if (![text hasPrefix:@"<"]) {
        if (text.length % 2) return nil;
        NSMutableData *decoded = [NSMutableData data];
        for (NSUInteger i = 0; i < text.length; i += 2) {
            unsigned value;
            NSScanner *scanner = [NSScanner scannerWithString:[text substringWithRange:NSMakeRange(i, 2)]];
            scanner.charactersToBeSkipped = nil;
            if (![scanner scanHexInt:&value] || !scanner.isAtEnd) return nil;
            uint8_t byte = (uint8_t)value;
            [decoded appendBytes:&byte length:1];
        }
        data = decoded;
    }
    id object = [NSPropertyListSerialization propertyListWithData:data options:NSPropertyListImmutable
                                                         format:nil error:nil];
    if (![object isKindOfClass:[NSDictionary class]]) return nil;
    id values = object[@"Partitions"];
    if (![values isKindOfClass:[NSArray class]] || [values count] > 64) return nil;
    for (id value in values) {
        if (![value isKindOfClass:[NSString class]] || [value length] > 1024) return nil;
    }
    return values;
}

int main(int argc, const char **argv) {
    @autoreleasepool {
        if (argc != 4 && argc != 5) return 2;
        NSMutableDictionary *report = [NSMutableDictionary dictionary];
        NSString *creatorHash = imageHash(argv[2]), *productHash = imageHash(argv[3]);
        if (!creatorHash || !productHash) return 4;
        report[@"different_image_cdhash"] = [creatorHash isEqualToString:productHash] ? @NO : @YES;
        OSStatus status = SecKeychainSetUserInteractionAllowed(false);
        report[@"interaction_disabled_status"] = @(status);
        if (status != errSecSuccess) return 3;
        NSString *account = [NSString stringWithUTF8String:argv[1]];
        NSString *service = [account hasPrefix:@"api:"] ? @"Cryptex-Dev-ManagedSyncthing" : @"Cryptex-Dev-DeviceCredentials";
        NSDictionary *query = @{
            (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
            (__bridge id)kSecAttrService: service,
            (__bridge id)kSecAttrAccount: account,
            (__bridge id)kSecReturnRef: @YES,
            (__bridge id)kSecMatchLimit: (__bridge id)kSecMatchLimitAll
        };
        CFTypeRef result = NULL;
        status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &result);
        report[@"lookup_status"] = @(status);
        if (status == errSecSuccess && CFGetTypeID(result) == CFArrayGetTypeID()) {
            CFIndex count = CFArrayGetCount((CFArrayRef)result);
            report[@"match_count"] = @(count);
            if (count == 1) {
                SecKeychainItemRef item = (SecKeychainItemRef)CFArrayGetValueAtIndex((CFArrayRef)result, 0);
                SecKeychainRef keychain = NULL, defaultKeychain = NULL;
                status = SecKeychainItemCopyKeychain(item, &keychain);
                report[@"containing_keychain_status"] = @(status);
                if (status == errSecSuccess) {
                    if (argc == 5) {
                        SecKeychainRef expected = NULL;
                        OSStatus opened = SecKeychainOpen(argv[4], &expected);
                        report[@"expected_keychain_status"] = @(opened);
                        if (opened == errSecSuccess) {
                            report[@"item_in_expected_keychain"] = CFEqual(keychain, expected) ? @YES : @NO;
                            CFRelease(expected);
                        }
                    }
                    SecKeychainStatus bits = 0;
                    status = SecKeychainGetStatus(keychain, &bits);
                    report[@"keychain_state_status"] = @(status);
                    if (status == errSecSuccess) report[@"keychain_unlocked"] = (bits & kSecUnlockStateStatus) ? @YES : @NO;
                    status = SecKeychainCopyDefault(&defaultKeychain);
                    report[@"default_keychain_status"] = @(status);
                    if (status == errSecSuccess) report[@"item_in_default_keychain"] = CFEqual(keychain, defaultKeychain) ? @YES : @NO;
                }
                SecAccessRef access = NULL;
                status = SecKeychainItemCopyAccess(item, &access);
                report[@"copy_access_status"] = @(status);
                if (status == errSecSuccess) {
                    CFArrayRef acls = SecAccessCopyMatchingACLList(access, kSecACLAuthorizationPartitionID);
                    report[@"partition_acl_count"] = @(acls ? CFArrayGetCount(acls) : 0);
                    if (acls && CFArrayGetCount(acls) == 1) {
                        CFStringRef description = NULL;
                        CFArrayRef apps = NULL;
                        SecKeychainPromptSelector selector = 0;
                        status = SecACLCopyContents((SecACLRef)CFArrayGetValueAtIndex(acls, 0), &apps, &description, &selector);
                        report[@"partition_contents_status"] = @(status);
                        NSArray *values = status == errSecSuccess ? partitions(description) : nil;
                        report[@"partition_format_recognized"] = values ? @YES : @NO;
                        if (values) {
                            report[@"partition_count"] = @(values.count);
                            report[@"creator_cdhash_allowed"] = [values containsObject:[@"cdhash:" stringByAppendingString:creatorHash]] ? @YES : @NO;
                            report[@"product_cdhash_allowed"] = [values containsObject:[@"cdhash:" stringByAppendingString:productHash]] ? @YES : @NO;
                        }
                    }
                }
            }
        }
        // Only literal field names, statuses, counts and booleans leave this process.
        NSData *json = [NSJSONSerialization dataWithJSONObject:report options:0 error:nil];
        fwrite(json.bytes, 1, json.length, stdout);
        putchar('\n');
    }
    return 0;
}
