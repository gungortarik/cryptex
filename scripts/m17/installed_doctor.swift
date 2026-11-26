// Read the Doctor message in the specific installed application under test.
import ApplicationServices
import Foundation

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}

guard CommandLine.arguments.count == 4, let pid = Int32(CommandLine.arguments[1]), pid > 0,
      AXIsProcessTrusted() else { exit(2) }
let expected = CommandLine.arguments[2], contradiction = CommandLine.arguments[3]
let application = AXUIElementCreateApplication(pid)
let deadline = ProcessInfo.processInfo.systemUptime + 35
repeat {
    var pending = attribute(application, "AXWindows") as? [AXUIElement] ?? []
    var found = false, conflicting = false
    while let item = pending.popLast() {
        if attribute(item, "AXRole") as? String == "AXStaticText" {
            let value = attribute(item, "AXValue") as? String
            found = found || value == expected
            conflicting = conflicting || value == contradiction
        }
        pending += attribute(item, "AXChildren") as? [AXUIElement] ?? []
    }
    if found && !conflicting { exit(0) }
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.25))
} while ProcessInfo.processInfo.systemUptime < deadline
fputs("The installed Desktop did not render the expected Doctor status.\n", stderr)
exit(1)
