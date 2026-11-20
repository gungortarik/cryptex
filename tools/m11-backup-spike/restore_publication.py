"""Publish a completed file or restore tree once inside a caller-owned sandbox."""

import ctypes
import os
import sys


def publish_exclusive(staged, destination):
    """Same-volume rename that refuses even an existing empty directory."""
    if os.name == "nt":
        os.rename(staged, destination)
        return

    libc = ctypes.CDLL(None, use_errno=True)
    source, target = os.fsencode(staged), os.fsencode(destination)
    if sys.platform == "darwin":
        result = libc.renamex_np(source, target, 4)  # RENAME_EXCL
    elif sys.platform.startswith("linux"):
        # AT_FDCWD and RENAME_NOREPLACE on Linux.
        result = libc.renameat2(-100, source, -100, target, 1)
    else:
        raise NotImplementedError("Exclusive directory publication unavailable on this platform")
    if result != 0:
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error), destination)
