"""Read kernel time spent suspended during this test, without power controls or logs."""
import ctypes
import sys
import time


def platform_clocks():
    if sys.platform == "darwin":
        lib = ctypes.CDLL("/usr/lib/libSystem.B.dylib")

        class Timebase(ctypes.Structure):
            _fields_ = [("numer", ctypes.c_uint32), ("denom", ctypes.c_uint32)]

        ratio = Timebase()
        lib.mach_timebase_info.argtypes = [ctypes.POINTER(Timebase)]
        if lib.mach_timebase_info(ctypes.byref(ratio)) != 0 or not ratio.denom or not ratio.numer:
            raise RuntimeError("mac_timebase_unavailable")

        def clock(name):
            fn = getattr(lib, name)
            fn.argtypes = []
            fn.restype = ctypes.c_uint64
            return lambda: fn() * ratio.numer // ratio.denom

        return clock("mach_continuous_time"), clock("mach_absolute_time")
    if sys.platform == "win32":
        lib = ctypes.WinDLL("api-ms-win-core-realtime-l1-1-1")

        def clock(name):
            fn = getattr(lib, name)
            fn.argtypes = [ctypes.POINTER(ctypes.c_ulonglong)]
            fn.restype = None

            def read():
                value = ctypes.c_ulonglong()
                fn(ctypes.byref(value))
                return value.value * 100

            return read

        return clock("QueryInterruptTimePrecise"), clock("QueryUnbiasedInterruptTimePrecise")
    raise RuntimeError("unsupported_os")


def sample(continuous, awake):
    # Bracket the second system call. Delayed scheduling widens the interval,
    # so it can only reduce our lower bound on time actually spent suspended.
    first = continuous()
    working = awake()
    last = continuous()
    if last < first:
        raise RuntimeError("clock_discontinuity")
    return first - working, last - working


def observe_sleep():
    clocks = platform_clocks()
    _, initial_upper = sample(*clocks)
    started_awake = clocks[1]()
    print("HAZIR. Bu test hesabinda bilgisayari uykuya alin, en az 10 saniye sonra uyandirin.", flush=True)
    while True:
        lower, _ = sample(*clocks)
        seconds = (lower - initial_upper) // 1_000_000_000
        if seconds >= 5:
            return seconds
        if clocks[1]() - started_awake >= 600 * 1_000_000_000:
            raise RuntimeError("sleep_not_observed")
        time.sleep(1)
