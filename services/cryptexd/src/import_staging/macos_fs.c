#include <sys/acl.h>
#include <errno.h>
#include <stdio.h>

/* A deny ACL does not confer write authority. Unknown/allow ACLs are refused;
 * this adapter never repairs or adopts existing permission state. */
int cx_import_acl_safe(int fd) {
    errno = 0;
    acl_t acl = acl_get_fd_np(fd, ACL_TYPE_EXTENDED);
    if (!acl) return errno == ENOENT || errno == ENOATTR ? 0 : -1;
    int result = 0;
    if (acl_valid(acl) != 0) result = -1;
    acl_entry_t entry;
    int selector = ACL_FIRST_ENTRY;
    while (!result) {
        errno = 0;
        int found = acl_get_entry(acl, selector, &entry);
        if (found != 0) {
            if (found != -1 || errno != EINVAL) result = -1;
            break;
        }
        acl_tag_t tag;
        if (acl_get_tag_type(entry, &tag) != 0 || tag != ACL_EXTENDED_DENY) result = -1;
        selector = ACL_NEXT_ENTRY;
    }
    if (acl_free(acl) != 0) result = -1;
    return result;
}

int cx_import_rename_exclusive(int source, const char *from, int target, const char *to) {
    return renameatx_np(source, from, target, to, RENAME_EXCL);
}
