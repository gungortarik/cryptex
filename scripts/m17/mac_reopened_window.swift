// Observe the specific Desktop reopened by the updater, then close its window normally.
import AppKit
import CoreGraphics
import Foundation

guard CommandLine.arguments.count == 3, let pid = Int32(CommandLine.arguments[2]), pid > 0 else {
    exit(2)
}
let executable = URL(fileURLWithPath: CommandLine.arguments[1]).resolvingSymlinksInPath()
let deadline = ProcessInfo.processInfo.systemUptime + 45
var observed: NSRunningApplication?
while observed == nil && ProcessInfo.processInfo.systemUptime < deadline {
    if let app = NSRunningApplication(processIdentifier: pid), app.isFinishedLaunching,
       app.executableURL?.resolvingSymlinksInPath() == executable,
       let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]],
       windows.contains(where: {
           ($0[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == pid &&
           ($0[kCGWindowLayer as String] as? NSNumber)?.intValue == 0
       }) {
        observed = app
    }
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
}
guard let app = observed, app.terminate() else {
    fputs("reopened_window_missing_or_quit_refused\n", stderr)
    exit(1)
}
let quitDeadline = ProcessInfo.processInfo.systemUptime + 15
while !app.isTerminated && ProcessInfo.processInfo.systemUptime < quitDeadline {
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
}
guard app.isTerminated else { exit(1) }
