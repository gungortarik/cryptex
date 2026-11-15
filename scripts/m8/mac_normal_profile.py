"""Exclusive synthetic fixture at the real default development-profile locator."""
import os
from pathlib import Path
import stat

from mac_identity_preflight import PreflightError


def identity(info):
    return info.st_dev, info.st_ino, info.st_uid, stat.S_IMODE(info.st_mode)


class DefaultProfile:
    def __init__(self, home):
        self.home = Path(home)
        self.path = self.home / 'Library/Application Support/Cryptex-Dev/runtime/p0-2'
        self.chain = []
        self.created = False

    def acquire(self):
        if not self.home.is_absolute() or self.home == Path('/'):
            raise PreflightError('normal_home_invalid')
        flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
        parent = os.open('/', flags)
        self.chain.append((None, '/', parent, identity(os.fstat(parent))))
        parts = self.path.parts[1:]
        home_depth = len(self.home.parts) - 1
        for depth, name in enumerate(parts, 1):
            leaf = depth == len(parts)
            if leaf:
                # mkdir refuses even an empty directory or dangling symlink.
                try:
                    os.mkdir(name, 0o700, dir_fd=parent)
                except FileExistsError:
                    raise PreflightError('normal_profile_already_exists') from None
                self.created = True
            elif depth > home_depth:
                try:
                    os.mkdir(name, 0o700, dir_fd=parent)
                except FileExistsError:
                    pass
            child = os.open(name, flags, dir_fd=parent)
            info = os.fstat(child)
            self.chain.append((parent, name, child, identity(info)))
            if (info.st_uid not in (0, os.getuid()) or stat.S_IMODE(info.st_mode) & 0o022
                    or (depth >= home_depth and info.st_uid != os.getuid())):
                raise PreflightError('normal_profile_ancestor_unsafe')
            parent = child
        self.check()
        return self.path

    def check(self):
        if not self.created or not self.chain:
            raise PreflightError('normal_profile_not_owned')
        for parent, name, fd, expected in self.chain:
            info = os.fstat(fd) if parent is None else os.stat(name, dir_fd=parent, follow_symlinks=False)
            if not stat.S_ISDIR(info.st_mode) or identity(info) != expected:
                raise PreflightError('normal_profile_path_changed')
        if self.chain[-1][3][3] != 0o700:
            raise PreflightError('normal_profile_permissions_changed')

    def remove(self):
        self.check()

        def walk(fd, delete):
            for name in os.listdir(fd):
                info = os.stat(name, dir_fd=fd, follow_symlinks=False)
                if info.st_uid != os.getuid():
                    raise PreflightError('normal_cleanup_foreign_object')
                if stat.S_ISDIR(info.st_mode):
                    child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
                    try:
                        if identity(os.fstat(child)) != identity(info):
                            raise PreflightError('normal_cleanup_object_changed')
                        walk(child, delete)
                    finally:
                        os.close(child)
                    if delete:
                        os.rmdir(name, dir_fd=fd)
                elif stat.S_ISREG(info.st_mode) and info.st_nlink == 1:
                    if delete:
                        os.unlink(name, dir_fd=fd)
                else:
                    raise PreflightError('normal_cleanup_unsafe_object')

        walk(self.chain[-1][2], False)
        self.check()
        walk(self.chain[-1][2], True)
        parent, name, _, _ = self.chain[-1]
        self.check()
        os.rmdir(name, dir_fd=parent)
        self.created = False

    def close(self):
        for _, _, fd, _ in reversed(self.chain):
            os.close(fd)
        self.chain.clear()
