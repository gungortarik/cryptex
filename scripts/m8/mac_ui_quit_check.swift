// Exercise AppKit's ordinary quit request against the exact child we spawned.
import AppKit
import Foundation

let desktop = Process()
desktop.executableURL = URL(fileURLWithPath: CommandLine.arguments[1])
desktop.arguments = ["--cryptex-m8-local-content-ui", CommandLine.arguments[2]]
try desktop.run()
let deadline = ProcessInfo.processInfo.systemUptime + 25
var requested = false
while desktop.isRunning && ProcessInfo.processInfo.systemUptime < deadline {
    if !requested,
       let app = NSRunningApplication(processIdentifier: desktop.processIdentifier),
       app.isFinishedLaunching {
        requested = app.terminate()
    }
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
}
guard requested, !desktop.isRunning, desktop.terminationReason == .exit,
      desktop.terminationStatus == 0 else {
    // The outer bounded test runner owns this entire process group on failure.
    fputs("native_ui_quit_failed\n", stderr)
    exit(1)
}
