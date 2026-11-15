/* Read-only acceptance observer. Never signal a discovered PID or read argv. */
#include <errno.h>
#include <libproc.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/proc.h>
#include <unistd.h>

#define LIMIT 4096
struct row { struct proc_bsdinfo info; int role; };
static pid_t pids[LIMIT];
static struct row rows[LIMIT];
static int info_size;

static int failed(const char *stage, int error, int observed) {
    /* Closed labels and numeric OS status only; no PID, path, argv or name. */
    printf("inventory_error %s %d %d\n", stage, error, observed);
    return 2;
}

static int info(pid_t pid, struct proc_bsdinfo *out) {
    errno = 0;
    int size = proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, out, sizeof(*out));
    info_size = size;
    if (size == (int)sizeof(*out)) return 1;
    return errno == ESRCH ? 0 : -1;
}

int main(int argc, char **argv) {
    if (argc < 4 || argc > 8) return failed("arguments", 0, 0);
    for (int i = 1; i < argc; ++i) if (argv[i][0] != '/') return failed("arguments", 0, 0);
    errno = 0;
    int size = proc_listpids(PROC_UID_ONLY, getuid(), pids, sizeof(pids));
    if (size <= 0 || size >= (int)sizeof(pids) || size % (int)sizeof(pid_t)) return failed("list", errno, size);
    int count = 0;
    for (int i = 0; i < size / (int)sizeof(pid_t); ++i) {
        if (pids[i] <= 0) continue;
        struct proc_bsdinfo a = {0}, b = {0};
        int found = info(pids[i], &a);
        if (found == 0) continue;
        if (found < 0) return failed("info", errno, info_size);
        if (a.pbi_uid != getuid()) return failed("uid", 0, 0);
        if (a.pbi_status == SZOMB) continue;
        char path[PROC_PIDPATHINFO_MAXSIZE] = {0};
        errno = 0;
        int path_size = proc_pidpath(pids[i], path, sizeof(path));
        int role = 0;
        if (path_size <= 0) {
            int path_error = errno;
            found = info(pids[i], &b);
            if (found == 0 || (found == 1 && b.pbi_status == SZOMB)) continue;
            if (path_error != ENOENT || found != 1) return failed("path", path_error, path_size);
            /* Preserve this live instance as opaque, never as absent. Python
             * may exclude only a frozen pre-spawn instance, not a new helper. */
            role = 4;
        }
        found = info(pids[i], &b);
        if (found == 0) continue;
        if (found < 0) return failed("recheck", errno, info_size);
        if (a.pbi_start_tvsec != b.pbi_start_tvsec ||
            a.pbi_start_tvusec != b.pbi_start_tvusec || a.pbi_uid != b.pbi_uid) return failed("changed", 0, 0);
        if (b.pbi_status == SZOMB) continue;
        if (role != 4) {
            for (int n = 1; n < argc; ++n) if (!strcmp(path, argv[n])) role = n < 3 ? n : 3;
        }
        rows[count++] = (struct row){b, role};
    }
    /* Export numeric metadata only. All unrelated rows stay in collector RAM. */
    for (int i = 0; i < count; ++i) {
        struct proc_bsdinfo *b = &rows[i].info;
        printf("%u %u %u %llu %llu %d\n", b->pbi_pid, b->pbi_ppid, b->pbi_pgid,
               (unsigned long long)b->pbi_start_tvsec,
               (unsigned long long)b->pbi_start_tvusec, rows[i].role);
    }
    return ferror(stdout) ? 2 : 0;
}
