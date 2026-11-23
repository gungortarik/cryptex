#import <Foundation/Foundation.h>
#include <stdint.h>
#include <string.h>

int cx_login_entry_matches(const uint8_t *bytes, size_t length, const char *desktop) {
    @autoreleasepool {
        NSData *data = [NSData dataWithBytes:bytes length:length];
        id plist = [NSPropertyListSerialization propertyListWithData:data options:NSPropertyListImmutable
                                                             format:NULL error:NULL];
        NSString *executable = [[NSFileManager defaultManager]
            stringWithFileSystemRepresentation:desktop length:strlen(desktop)];
        if (executable == nil) return 0;
        NSDictionary *expected = @{@"Label": @"Cryptex", @"ProgramArguments": @[executable, @"--cryptex-background"],
                                   @"RunAtLoad": @YES};
        return [expected isEqual:plist] ? 1 : 0;
    }
}

// Trash is recoverable and uses the OS-selected location for the file's volume.
int cx_move_to_trash(const char *path) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithFileSystemRepresentation:path isDirectory:NO relativeToURL:nil];
        NSError *error = nil;
        if ([[NSFileManager defaultManager] trashItemAtURL:url resultingItemURL:NULL error:&error]) return 0;
        return error.code ? (int)error.code : -1;
    }
}
