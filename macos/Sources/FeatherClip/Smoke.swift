import AppKit

// Exercises the real native responder chain using only synthetic demo history.
extension AppDelegate {
    func smokeStep(_ timer: Timer) {
        if smokePhase == 0 && clips.count == 5 {
            // Exercise closing and reopening before using the real responder chain.
            hide()
            show()
            smokePhase = 11
        } else if smokePhase == 11 {
            guard NSApp.isActive, panel.isKeyWindow,
                search.currentEditor() === panel.firstResponder
            else {
                smokeFocusAttempts += 1
                if smokeFocusAttempts >= 10 {
                    fputs(
                        "SMOKE FAIL: opening popup focus (active=\(NSApp.isActive), key=\(panel.isKeyWindow), search=\(search.currentEditor() === panel.firstResponder), pending=\(focusPending))\n",
                        stderr)
                    exit(1)
                }
                return
            }
            guard panel.isVisible, table.numberOfRows == 5, panel.tabbingMode == .disallowed else {
                exit(1)
            }
            if #available(macOS 26.0, *) { guard nativeGlass else { exit(1) } }
            smokeKey(125)
            guard panel.firstResponder === table, table.selectedRow == 0 else {
                fputs("SMOKE FAIL: Down did not move search focus to the first clip\n", stderr)
                exit(1)
            }
            smokeKey(125)
            smokeRequire(table.selectedRow == 1, "Down selects the next clip")
            let selectedID = visible[1].id
            clips.swapAt(0, 1)
            reload()
            smokeRequire(
                visible[table.selectedRow].id == selectedID, "History refresh preserves selection")
            smokeKey(36, modifiers: .shift)
            smokeRequire(busy, "Shift+Enter copies the selected clip")
            smokeKey(48)
            guard panel.firstResponder === pause else {
                fputs("SMOKE FAIL: Tab did not move keyboard focus from clips to Pause\n", stderr)
                exit(1)
            }
            smokeKey(48)
            smokeRequire(panel.firstResponder === clear, "Tab to Clear")
            smokeKey(48)
            smokeRequire(panel.firstResponder === more, "Tab to options")
            smokeKey(48, modifiers: .shift)
            smokeRequire(panel.firstResponder === clear, "Shift+Tab to Clear")
            smokeKey(48, modifiers: .shift)
            smokeRequire(panel.firstResponder === pause, "Shift+Tab to Pause")
            smokeKey(49)
            search.stringValue = "سلام"
            reload()
            guard visible.count == 1 else { exit(1) }
            search.stringValue = ""
            reload()
            smokePhase = 1
        } else if smokePhase == 1 && paused {
            smokePhase = 2
            service.clear()
        } else if smokePhase == 2 && clips.isEmpty {
            hide()
            show()
            guard panel.isVisible, visible.isEmpty else { exit(1) }
            smokePhase = 3
        } else if smokePhase == 3 && NSApp.isActive
            && search.currentEditor() === panel.firstResponder
        {
            smokeKey(48)
            smokeRequire(panel.firstResponder === pause, "Empty history: Tab skips clips")
            smokeKey(48)
            smokeRequire(panel.firstResponder === more, "Empty history: Tab skips disabled Clear")
            timer.invalidate()
            print(
                "SMOKE PASS: native AppKit, activation/search focus, arrows, Tab/Shift+Tab, keyboard buttons, empty history, copy, pause, clear, popup"
            )
            NSApp.terminate(nil)
        }
    }
    func smokeRequire(_ condition: Bool, _ message: String) {
        if !condition {
            fputs("SMOKE FAIL: \(message)\n", stderr)
            exit(1)
        }
    }
    func smokeKey(_ code: UInt16, modifiers: NSEvent.ModifierFlags = []) {
        let characters =
            [UInt16(125): "\u{f701}", 48: "\t", 49: " ", 36: "\r", 53: "\u{1b}"][code] ?? ""
        let event = NSEvent.keyEvent(
            with: .keyDown, location: .zero, modifierFlags: modifiers,
            timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: panel.windowNumber,
            context: nil, characters: characters, charactersIgnoringModifiers: characters,
            isARepeat: false,
            keyCode: code)!
        panel.sendEvent(event)
    }
}
