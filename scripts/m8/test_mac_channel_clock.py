"""Exercise production C deadlines with independently advancing Darwin clocks."""
import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
CHANNEL = ROOT / 'services/cryptexd/src/m8_owner_channel/native/channel.c'

HARNESS = r'''
#include <time.h>
#include <errno.h>
#include <stdio.h>
static int fixture_clock_gettime(clockid_t, struct timespec *);
#define clock_gettime fixture_clock_gettime
#include "channel_under_test.c"
#undef clock_gettime

static uint64_t awake = 100000, continuous = 500000;
static int clock_failure = 0;
static int fixture_clock_gettime(clockid_t id, struct timespec *t) {
    if (clock_failure) { errno = EINVAL; return -1; }
    uint64_t ms;
    if (id == CLOCK_UPTIME_RAW) ms = awake;
    else if (id == CLOCK_MONOTONIC) ms = continuous;
    else return -1;
    t->tv_sec = (time_t)(ms / 1000);
    t->tv_nsec = (long)(ms % 1000) * 1000000;
    return 0;
}
#define CHECK(condition) do { if (!(condition)) { \
    fprintf(stderr, "clock regression at line %d\n", __LINE__); return 1; \
} } while (0)
int main(void) {
    // Rust caller's four-second remaining budget is translated at entry.
    uint64_t deadline = cx_now_ms() + 4000;
    CHECK(remaining_ms(deadline) == 4000);
    awake += 750; continuous += 750;
    CHECK(remaining_ms(deadline) == 3250);
    // One hour of machine sleep consumes no Rust Instant / Mach wait budget.
    continuous += 3600000;
    CHECK(remaining_ms(deadline) == 3250);
    // After wake, genuine awake-time expiry is still exact and fails closed.
    awake += 3249; continuous += 3249;
    CHECK(remaining_ms(deadline) == 1);
    awake++; continuous++;
    CHECK(remaining_ms(deadline) == 0);
    CHECK(remaining_ms(0) == 0);
    CHECK(remaining_ms(cx_now_ms() + (uint64_t)UINT32_MAX + 1) == 0);
    CHECK(remaining_ms(deadline_ms()) == WAIT_MS);
    // Clock failure must not accidentally extend a request or handshake.
    clock_failure = 1;
    CHECK(cx_now_ms() == UINT64_MAX);
    CHECK(remaining_ms(deadline) == 0);
    CHECK(deadline_ms() == 0);
    return 0;
}
'''


@unittest.skipUnless(sys.platform == 'darwin', 'requires Darwin Security/Mach headers')
class ChannelClockTests(unittest.TestCase):
    def execute(self, source):
        with tempfile.TemporaryDirectory(prefix='cryptex-clock-') as directory:
            root = pathlib.Path(directory)
            (root / 'channel_under_test.c').write_text(source, encoding='utf-8')
            (root / 'clock_test.c').write_text(HARNESS, encoding='utf-8')
            binary = root / 'clock-test'
            subprocess.run(['/usr/bin/clang', '-std=c11', '-Wall', '-Wextra', '-Werror',
                            '-I', str(CHANNEL.parent), str(root / 'clock_test.c'),
                            '-framework', 'Security', '-framework', 'CoreFoundation',
                            '-lbsm', '-o', str(binary)], check=True, timeout=60,
                           capture_output=True)
            return subprocess.run([str(binary)], capture_output=True, timeout=10)

    def test_sleep_does_not_consume_rust_budget_but_awake_time_does(self):
        result = self.execute(CHANNEL.read_text(encoding='utf-8'))
        self.assertEqual(result.returncode, 0, result.stderr.decode())

    def test_previous_mixed_clock_fails_the_same_sleep_scenario(self):
        source = CHANNEL.read_text(encoding='utf-8')
        self.assertEqual(source.count('clock_gettime(CLOCK_UPTIME_RAW, &t)'), 1)
        result = self.execute(source.replace('clock_gettime(CLOCK_UPTIME_RAW, &t)',
                                             'clock_gettime(CLOCK_MONOTONIC, &t)'))
        self.assertEqual(result.returncode, 1)
        self.assertIn(b'clock regression', result.stderr)


if __name__ == '__main__':
    unittest.main()
