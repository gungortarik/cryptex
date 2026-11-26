// An isolated WebKit form for checking native text entry before building either release.
import AppKit
import WebKit

let application = NSApplication.shared
application.setActivationPolicy(.regular)
let menu = NSMenu()
let edit = NSMenu(title: "Edit")
edit.addItem(withTitle: "Select All", action: Selector(("selectAll:")), keyEquivalent: "a")
edit.addItem(withTitle: "Paste", action: Selector(("paste:")), keyEquivalent: "v")
let editItem = NSMenuItem(title: "Edit", action: nil, keyEquivalent: "")
menu.addItem(editItem)
editItem.submenu = edit
application.mainMenu = menu
let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 700, height: 420),
                      styleMask: [.titled, .closable], backing: .buffered, defer: false)
let web = WKWebView(frame: window.contentView!.bounds)
window.contentView = web
web.loadHTMLString(#"""
    <label for="proof">Public proof</label><textarea id="proof"></textarea>
    <div style="height:850px"></div>
    <label for="transcript">Public transcript</label><textarea id="transcript">replace me</textarea>
    <button disabled>Fields received both edits</button>
    <script>
      document.addEventListener("input", () => {
        document.querySelector("button").disabled =
          document.querySelector("#proof").value !== "synthetic proof" ||
          document.querySelector("#transcript").value !== "line one\nline two";
      });
    </script>
    """#, baseURL: nil)
window.center()
window.makeKeyAndOrderFront(nil)
application.activate(ignoringOtherApps: true)
application.run()
