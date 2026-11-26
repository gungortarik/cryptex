// Drives only the historical Desktop in the disposable hosted login session.
import AppKit
import ApplicationServices
import Foundation

struct TrialError: Error {
    let stage: String
    init(_ stage: String) { self.stage = stage }
}

func attribute(_ element: AXUIElement, _ key: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, key as CFString, &value) == .success else { return nil }
    return value
}

struct Window {
    let app: AXUIElement
    let pid: pid_t

    func find(_ role: String, _ label: String) -> AXUIElement? {
        var pending = attribute(app, "AXWindows") as? [AXUIElement] ?? []
        var count = 0
        while let element = pending.popLast(), count < 3000 {
            count += 1
            if attribute(element, "AXRole") as? String == role {
                let matches = ["AXTitle", "AXDescription", "AXDOMIdentifier", "AXIdentifier", "AXValue"]
                    .contains { (attribute(element, $0) as? String) == label }
                if matches { return element }
            }
            pending += attribute(element, "AXChildren") as? [AXUIElement] ?? []
        }
        return nil
    }

    func control(_ role: String, _ labels: [String]) throws -> AXUIElement {
        let stop = ProcessInfo.processInfo.systemUptime + 20
        repeat {
            for label in labels {
                if let element = find(role, label),
                   (attribute(element, "AXEnabled") as? Bool) != false { return element }
            }
            Thread.sleep(forTimeInterval: 0.2)
        } while ProcessInfo.processInfo.systemUptime < stop
        throw TrialError("control_" + labels[0])
    }

    func press(_ role: String, _ label: String) throws {
        let element = try control(role, [label])
        guard AXUIElementPerformAction(element, kAXPressAction as CFString) == .success
        else { throw TrialError("press_" + label) }
    }

    func publicText(_ id: String, _ label: String) throws -> String {
        let box = try control("AXTextArea", [id, label])
        guard let value = attribute(box, "AXValue") as? String,
              !value.isEmpty, value.utf8.count <= 65536 else { throw TrialError("read_" + id) }
        return value
    }

    func fill(_ id: String, _ label: String, _ value: String) throws {
        let box = try control("AXTextArea", [id, label])
        // Use the same paste operation as the owner. A WebKit AXValue write
        // does not reliably deliver an edit to a React-controlled textarea.
        guard !value.isEmpty, value.utf8.count <= 262147 else { throw TrialError("input_size_" + id) }
        func focused() -> Bool {
            guard let element = attribute(app, "AXFocusedUIElement") else { return false }
            return CFEqual(element, box)
        }
        if !focused() {
            _ = AXUIElementSetAttributeValue(box, kAXFocusedAttribute as CFString, kCFBooleanTrue)
            let deadline = ProcessInfo.processInfo.systemUptime + 5
            while !focused() && ProcessInfo.processInfo.systemUptime < deadline {
                Thread.sleep(forTimeInterval: 0.05)
            }
        }
        guard focused() else { throw TrialError("focus_" + id) }
        func shortcut(_ key: CGKeyCode) throws {
            guard focused(),
                  let down = CGEvent(keyboardEventSource: nil, virtualKey: key, keyDown: true),
                  let up = CGEvent(keyboardEventSource: nil, virtualKey: key, keyDown: false)
            else { throw TrialError("key_" + id) }
            for event in [down, up] {
                event.flags = .maskCommand
                event.postToPid(pid)
            }
        }
        let clipboard = NSPasteboard.general
        clipboard.clearContents()
        guard clipboard.setString(value, forType: .string) else { throw TrialError("clipboard_" + id) }
        let change = clipboard.changeCount
        defer { if clipboard.changeCount == change { clipboard.clearContents() } }
        try shortcut(0) // Command-A replaces the prefilled transcript as well.
        try shortcut(9) // Command-V triggers the normal input event.
        let deadline = ProcessInfo.processInfo.systemUptime + 8
        repeat {
            if attribute(box, "AXValue") as? String == value { return }
            Thread.sleep(forTimeInterval: 0.1)
        } while ProcessInfo.processInfo.systemUptime < deadline
        throw TrialError("paste_" + id)
    }
}

func run() throws {
    let args = CommandLine.arguments
    guard args.count == 8, ProcessInfo.processInfo.environment["RUNNER_ENVIRONMENT"] == "github-hosted",
          AXIsProcessTrusted() else { throw TrialError("hosted_accessibility_unavailable") }
    let desktop = URL(fileURLWithPath: args[1]).standardizedFileURL
    let expected = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Applications/Cryptex.app/Contents/MacOS/cryptex-desktop")
    guard desktop == expected, desktop.resolvingSymlinksInPath() == expected else { throw TrialError("installed_path") }
    let process = Process()
    process.executableURL = desktop
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    try process.run()
    defer { if process.isRunning { NSRunningApplication(processIdentifier: process.processIdentifier)?.terminate() } }
    let app = AXUIElementCreateApplication(process.processIdentifier)
    let window = Window(app: app, pid: process.processIdentifier)
    func response(_ messages: [String]) throws -> String {
        let child = Process()
        child.executableURL = URL(fileURLWithPath: args[2])
        child.arguments = Array(args[3...6])
        let input = Pipe(), output = Pipe()
        child.standardInput = input
        child.standardOutput = output
        try child.run()
        input.fileHandleForWriting.write(Data(messages.joined(separator: "\n").utf8))
        input.fileHandleForWriting.closeFile()
        let data = output.fileHandleForReading.readDataToEndOfFile()
        child.waitUntilExit()
        guard child.terminationStatus == 0, (1...65536).contains(data.count),
              let value = String(data: data, encoding: .utf8) else { throw TrialError("public_peer_refused") }
        return value
    }

    try window.press("AXButton", "Create signed offer")
    let offer = try window.publicText("bootstrap-offer-output", "1. Signed offer · send to the responder")
    let proof = try response([offer])
    try window.fill("bootstrap-proof-input", "2. Signed proof · paste from the responder", proof)
    try window.press("AXButton", "Verify messages and review pairing")
    _ = try window.control("AXStaticText", ["44444444-4444-4444-8444-444444444444"])
    try window.press("AXButton", "Approve and sign")
    let approval = try window.publicText("bootstrap-signed-message", "3. Signed approval · send to the responder")
    let last = try response([offer, proof, approval])
    try window.fill("bootstrap-complete-transcript", "Append the responder’s signed approval on line 4",
                    [offer, proof, approval, last].joined(separator: "\n"))
    try window.press("AXButton", "Add approved device to local trust")
    _ = try window.control("AXStaticText", ["Pairing recorded in canonical local history. Device sync is not started by this action."])
    try window.press("AXButton", "Connection settings")
    try window.press("AXButton", "Reload current settings")
    let pause = try window.control("AXCheckBox", ["Pause sync"])
    func waitForSavedPause(_ expected: Bool) throws {
        let settings = URL(fileURLWithPath: args[7])
        let until = ProcessInfo.processInfo.systemUptime + 45
        repeat {
            if let data = try? Data(contentsOf: settings),
               let object = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
               object["paused"] as? Bool == expected { return }
            Thread.sleep(forTimeInterval: 0.2)
        } while ProcessInfo.processInfo.systemUptime < until
        throw TrialError("settings_not_persisted")
    }
    // Persist a paused local-only policy through the regular settings form.
    guard let wasPaused = attribute(pause, "AXValue") as? NSNumber else { throw TrialError("pause_value") }
    if wasPaused.boolValue {
        try window.press("AXCheckBox", "Pause sync")
        try window.press("AXButton", "Save connection settings")
        try waitForSavedPause(false)
    }
    try window.press("AXCheckBox", "Pause sync")
    try window.press("AXButton", "Save connection settings")
    try waitForSavedPause(true)
    guard NSRunningApplication(processIdentifier: process.processIdentifier)?.terminate() == true
    else { throw TrialError("quit_refused") }
    process.waitUntilExit()
    guard process.terminationStatus == 0 else { throw TrialError("desktop_exit") }
}

do { try run() } catch {
    fputs("installed_pair_window=" + ((error as? TrialError)?.stage ?? "process_error") + "\n", stderr)
    exit(1)
}
