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

static int info(pid_t pid, struct proc_bsdinfo *out) {
    errno = 0;
    int size = proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, out, sizeof(*out));
    if (size == (int)sizeof(*out)) return 1;
    return errno == ESRCH ? 0 : -1;
}

int main(int argc, char **argv) {
    if (argc < 4 || argc > 8) return 2;
    for (int i = 1; i < argc; ++i) if (argv[i][0] != '/') return 2;
    int size = proc_listpids(PROC_UID_ONLY, getuid(), pids, sizeof(pids));
    if (size <= 0 || size >= (int)sizeof(pids) || size % (int)sizeof(pid_t)) return 2;
    int count = 0;
    for (int i = 0; i < size / (int)sizeof(pid_t); ++i) {
        if (pids[i] <= 0) continue;
        struct proc_bsdinfo a = {0}, b = {0};
        int found = info(pids[i], &a);
        if (found == 0) continue;
        if (found < 0 || a.pbi_uid != getuid()) return 2;
        if (a.pbi_status == SZOMB) continue;
        char path[PROC_PIDPATHINFO_MAXSIZE] = {0};
        if (proc_pidpath(pids[i], path, sizeof(path)) <= 0) {
            found = info(pids[i], &b);
            if (found == 0 || (found == 1 && b.pbi_status == SZOMB)) continue;
            return 2;
        }
        found = info(pids[i], &b);
        if (found == 0) continue;
        if (found < 0 || a.pbi_start_tvsec != b.pbi_start_tvsec ||
            a.pbi_start_tvusec != b.pbi_start_tvusec || a.pbi_uid != b.pbi_uid) return 2;
        if (b.pbi_status == SZOMB) continue;
        int role = 0;
        for (int n = 1; n < argc; ++n) if (!strcmp(path, argv[n])) role = n < 3 ? n : 3;
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
