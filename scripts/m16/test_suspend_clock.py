import time
import unittest

from suspend_clock import platform_clocks, sample


class SuspendedTime(unittest.TestCase):
    def test_measurement_delay_alone_cannot_produce_sleep(self):
        before = sample(iter((10, 13)).__next__, lambda: 9)
        # Deliberately delay *both* sides of the inner measurement.
        after = sample(iter((20, 90)).__next__, lambda: 64)
        self.assertLessEqual(after[0] - before[1], 0)

    def test_only_sleep_after_initial_bracket_is_counted(self):
        before = sample(iter((1000, 1010)).__next__, lambda: 100)
        after = sample(iter((2700, 2702)).__next__, lambda: 300)
        self.assertEqual(after[0] - before[1], 1490)

    def test_native_awake_sample(self):
        clocks = platform_clocks()
        first = sample(*clocks)
        time.sleep(.05)
        last = sample(*clocks)
        self.assertLessEqual(first[0], first[1])
        self.assertLessEqual(last[0], last[1])
        self.assertLess(last[0] - first[1], 5_000_000_000)


if __name__ == "__main__":
    unittest.main()
