// Product-separated signed fixture policy. No product IPC activation.
#ifndef CPX_BOOTSTRAP_BUNDLE_H
#define CPX_BOOTSTRAP_BUNDLE_H
#include <libproc.h>
#ifndef CPX_BUNDLE_IDENTITY
#define CPX_BUNDLE_IDENTITY 0
#endif
#if CPX_BUNDLE_IDENTITY != 0 && CPX_BUNDLE_IDENTITY != 1
#error invalid bundle policy selection
#endif
#define BOOTSTRAP_DESKTOP_ID "com.cryptex.m8.bootstrap.desktop"
#define BOOTSTRAP_DAEMON_ID "com.cryptex.m8.bootstrap.daemon"
static char peer_executable[4096];
static const char *bundle_diagnostic = "not_requested_ci_mechanism_only";
static bool bundle_is_desktop;

#if CPX_BUNDLE_IDENTITY
static SecRequirementRef bundle_requirement(const char *identifier, const char *team) {
    char text[512];
    if (snprintf(text, sizeof(text), "identifier \"%s\" and anchor apple generic and certificate leaf[subject.OU] = \"%s\"", identifier, team) >= (int)sizeof(text)) return NULL;
    CFStringRef string=CFStringCreateWithCString(NULL, text, kCFStringEncodingUTF8);
    SecRequirementRef requirement=NULL;
    if (string) {
        if (SecRequirementCreateWithString(string, 0, &requirement) != errSecSuccess) requirement=NULL;
        CFRelease(string);
    }
    return requirement;
}
static SecStaticCodeRef bundle_image(const char *path, SecRequirementRef expected, bool nested) {
    CFURLRef url=CFURLCreateFromFileSystemRepresentation(NULL, (const UInt8 *)path, strlen(path), false);
    SecStaticCodeRef image=NULL;
    SecCSFlags flags=kSecCSStrictValidate | kSecCSCheckAllArchitectures;
    if (nested) flags |= kSecCSCheckNestedCode;
    if (url && (SecStaticCodeCreateWithPath(url, 0, &image) != errSecSuccess || !image ||
        SecStaticCodeCheckValidity(image, flags, expected) != errSecSuccess)) {
        if (image) CFRelease(image);
        image=NULL;
    }
    if (url) CFRelease(url);
    return image;
}
static bool bundle_identity_open(void) {
    char path[4096]={0}, bundle_path[4096]={0};
    SecCodeRef self=NULL;
    SecRequirementRef initial=NULL, bundle_req=NULL;
    CFStringRef initial_text=NULL;
    CFDictionaryRef info=NULL;
    SecStaticCodeRef bundle=NULL, disk=NULL, peer=NULL;
    CFDataRef live_hash=NULL, disk_hash=NULL;
    bool valid=false;
    bundle_diagnostic="packaged_self_path_refused";
    int count=proc_pidpath(getpid(), path, sizeof(path));
    if (count <= 0 || count >= (int)sizeof(path)) goto done;
    const char *desktop_suffix="/Contents/MacOS/Desktop", *daemon_suffix="/Contents/MacOS/Daemon";
    size_t length=strlen(path), tail=strlen(desktop_suffix);
    bool is_desktop=length > tail && !strcmp(path+length-tail, desktop_suffix);
    bundle_is_desktop=is_desktop;
    if (!is_desktop) {
        tail=strlen(daemon_suffix);
        if (length <= tail || strcmp(path+length-tail, daemon_suffix)) goto done;
    }
    if (length-tail >= sizeof(bundle_path)) goto done;
    memcpy(bundle_path, path, length-tail);
    size_t bundle_length=strlen(bundle_path);
    if (bundle_length < 4 || strcmp(bundle_path+bundle_length-4, ".app")) goto done;
    const char *own_id=is_desktop ? BOOTSTRAP_DESKTOP_ID : BOOTSTRAP_DAEMON_ID;
    const char *peer_id=is_desktop ? BOOTSTRAP_DAEMON_ID : BOOTSTRAP_DESKTOP_ID;
    initial_text=CFStringCreateWithFormat(NULL, NULL, CFSTR("identifier \"%s\" and anchor apple generic"), own_id);
    bundle_diagnostic="apple_anchor_or_fixed_self_identifier_refused";
    if (!initial_text || SecRequirementCreateWithString(initial_text, 0, &initial) != errSecSuccess ||
        SecCodeCopySelf(0, &self) != errSecSuccess || !self ||
        SecCodeCheckValidity(self, kSecCSStrictValidate, initial) != errSecSuccess ||
        SecCodeCopySigningInformation((SecStaticCodeRef)self, kSecCSSigningInformation, &info) != errSecSuccess || !info) goto done;
    bundle_diagnostic="certified_team_unavailable";
    CFTypeRef team_value=CFDictionaryGetValue(info, kSecCodeInfoTeamIdentifier);
    char team[64];
    if (!team_value || CFGetTypeID(team_value) != CFStringGetTypeID() ||
        !CFStringGetCString(team_value, team, sizeof(team), kCFStringEncodingASCII) || strlen(team) != 10) goto done;
    for (size_t i=0; i<10; ++i) if (!((team[i]>='A' && team[i]<='Z') || (team[i]>='0' && team[i]<='9'))) goto done;
    own_requirement=bundle_requirement(own_id, team);
    fixture_requirement=bundle_requirement(peer_id, team);
    bundle_req=bundle_requirement(BOOTSTRAP_DESKTOP_ID, team);
    if (!own_requirement || !fixture_requirement || !bundle_req ||
        SecCodeCheckValidity(self, kSecCSStrictValidate, own_requirement) != errSecSuccess) goto done;
    live_hash=hash_of(self);
    if (snprintf(peer_executable, sizeof(peer_executable), "%s%s", bundle_path, is_desktop ? daemon_suffix : desktop_suffix) >= (int)sizeof(peer_executable)) goto done;
    bundle_diagnostic="sealed_bundle_or_counterpart_refused";
    bundle=bundle_image(bundle_path, bundle_req, true);
    disk=bundle_image(path, own_requirement, false);
    peer=bundle_image(peer_executable, fixture_requirement, false);
    if (!bundle || !disk || !peer || !live_hash) goto done;
    disk_hash=hash_of((SecCodeRef)disk);
    fixture_hash=hash_of((SecCodeRef)peer);
    if (!disk_hash || !fixture_hash || !CFEqual(live_hash, disk_hash) ||
        SecCodeCheckValidity(self, kSecCSStrictValidate, own_requirement) != errSecSuccess) goto done;
    own_hash=CFRetain(live_hash);
    valid=true;
    bundle_diagnostic="verified_self_organization_two_binary_bundle";
done:
    if (initial_text) CFRelease(initial_text);
    if (initial) CFRelease(initial);
    if (bundle_req) CFRelease(bundle_req);
    if (self) CFRelease(self);
    if (info) CFRelease(info);
    if (bundle) CFRelease(bundle);
    if (disk) CFRelease(disk);
    if (peer) CFRelease(peer);
    if (live_hash) CFRelease(live_hash);
    if (disk_hash) CFRelease(disk_hash);
    return valid;
}
#endif
#endif
