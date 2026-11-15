import AppKit
import ApplicationServices
import Carbon
import FeatherClipCore
import ImageIO
import ServiceManagement

final class ClipboardPanel: NSPanel {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}
final class ClipTable: NSTableView {
    var choose: ((Bool) -> Void)?
    var dismiss: (() -> Void)?
    override func keyDown(with event: NSEvent) {
        if [36, 76].contains(event.keyCode) {
            choose?(event.modifierFlags.contains(.shift))
        } else if event.keyCode == 53 {
            dismiss?()
        } else {
            super.keyDown(with: event)
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSTableViewDataSource,
    NSTableViewDelegate, NSSearchFieldDelegate, NSWindowDelegate, NSMenuDelegate
{
    let demo =
        CommandLine.arguments.contains("--demo") || CommandLine.arguments.contains("--smoke-test")
    let smoke = CommandLine.arguments.contains("--smoke-test")
    let keepOpen = CommandLine.arguments.contains("--verify-paste")
    lazy var service = ClipboardService(demo: demo)
    var item: NSStatusItem!
    var panel: ClipboardPanel!
    let search = NSSearchField()
    let table = ClipTable()
    let status = NSTextField(labelWithString: "Listening")
    let hint = NSTextField(labelWithString: "")
    let pause = NSButton(title: "Pause", target: nil, action: nil)
    let clear = NSButton(title: "Clear", target: nil, action: nil)
    var clips: [Clip] = []
    var visible: [Clip] = []
    var paused = false
    var target: NSRunningApplication?
    var previous: NSRunningApplication?
    var thumbnails: [Int: NSImage] = [:]
    var hotkey: EventHotKeyRef?
    var eventHandler: EventHandlerRef?
    var generation = 0
    var busy = false
    var menuOpen = false
    var nativeGlass = false
    var smokePhase = 0

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        NSWindow.allowsAutomaticWindowTabbing = false
        if !demo,
            let other = NSRunningApplication.runningApplications(
                withBundleIdentifier: "io.github.AK6.FeatherClip"
            ).first(where: { $0.processIdentifier != getpid() })
        {
            DistributedNotificationCenter.default().post(
                name: .init("io.github.AK6.FeatherClip.show"), object: nil)
            other.activate(options: [])
            NSApp.terminate(nil)
            return
        }
        previous = NSWorkspace.shared.frontmostApplication
        NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: .main
        ) { [weak self] note in
            guard
                let app = note.userInfo?[NSWorkspace.applicationUserInfoKey]
                    as? NSRunningApplication, app.processIdentifier != getpid()
            else { return }
            MainActor.assumeIsolated { self?.previous = app }
        }
        DistributedNotificationCenter.default().addObserver(
            self, selector: #selector(showNotification),
            name: .init("io.github.AK6.FeatherClip.show"), object: nil)
        makePanel()
        item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.image = NSImage(
            systemSymbolName: "clipboard", accessibilityDescription: "Clipboard history")
        item.button?.target = self
        item.button?.action = #selector(toggle)
        registerShortcut()
        service.onChange = { [weak self] snapshot in self?.apply(snapshot) }
        service.start(
            image: Bundle.main.url(forResource: "sample", withExtension: "png").flatMap {
                try? Data(contentsOf: $0)
            })
        if !CommandLine.arguments.contains("--background") { show() }
        if smoke {
            Timer.scheduledTimer(withTimeInterval: 0.15, repeats: true) { [weak self] timer in
                MainActor.assumeIsolated { self?.smokeStep(timer) }
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 15) {
                fputs("SMOKE FAIL: native AppKit timed out\n", stderr)
                exit(1)
            }
        }
    }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool)
        -> Bool
    {
        show()
        return false
    }
    func applicationWillTerminate(_ notification: Notification) {
        if let hotkey { UnregisterEventHotKey(hotkey) }
        if let eventHandler { RemoveEventHandler(eventHandler) }
    }
    private func makePanel() {
        panel = ClipboardPanel(
            contentRect: .init(x: 0, y: 0, width: 360, height: 430),
            styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.title = "FeatherClip"
        panel.delegate = self
        panel.level = .floating
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.collectionBehavior = [
            .canJoinAllSpaces, .fullScreenAuxiliary, .transient, .ignoresCycle,
        ]
        panel.tabbingMode = .disallowed
        panel.animationBehavior = .utilityWindow
        let content = NSView(frame: panel.contentView!.bounds)
        content.autoresizingMask = [.width, .height]
        if #available(macOS 26.0, *) {
            let glass = NSGlassEffectView(frame: content.bounds)
            glass.style = .regular
            glass.cornerRadius = 16
            glass.contentView = content
            panel.contentView = glass
            nativeGlass = true
        } else {
            let material = NSVisualEffectView(frame: content.bounds)
            material.material = .popover
            material.blendingMode = .behindWindow
            material.state = .active
            material.wantsLayer = true
            material.layer?.cornerRadius = 14
            material.layer?.masksToBounds = true
            material.addSubview(content)
            panel.contentView = material
        }
        let title = NSTextField(labelWithString: "Clipboard")
        title.font = .systemFont(ofSize: 13, weight: .semibold)
        let close = NSButton(
            image: NSImage(systemSymbolName: "xmark", accessibilityDescription: "Close clipboard")!,
            target: self, action: #selector(dismiss))
        close.isBordered = false
        close.toolTip = "Close (Esc)"
        let heading = NSStackView(views: [title, NSView(), close])
        heading.orientation = .horizontal
        search.placeholderString = "Search clips"
        search.delegate = self
        search.sendsSearchStringImmediately = true
        search.setAccessibilityLabel("Search clipboard history")
        let column = NSTableColumn(identifier: .init("clip"))
        column.width = 320
        table.addTableColumn(column)
        table.headerView = nil
        table.rowHeight = 54
        table.backgroundColor = .clear
        table.style = .fullWidth
        table.dataSource = self
        table.delegate = self
        table.target = self
        table.action = #selector(rowClicked)
        table.choose = { [weak self] in self?.choose(copyOnly: $0) }
        table.dismiss = { [weak self] in self?.dismiss() }
        let scroll = NSScrollView()
        scroll.documentView = table
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        scroll.borderType = .noBorder
        status.font = .systemFont(ofSize: 10)
        status.textColor = .secondaryLabelColor
        hint.font = .systemFont(ofSize: 10)
        hint.textColor = .secondaryLabelColor
        hint.lineBreakMode = .byWordWrapping
        hint.maximumNumberOfLines = 2
        pause.target = self
        pause.action = #selector(togglePause)
        clear.target = self
        clear.action = #selector(clearHistory)
        for button in [pause, clear] { button.bezelStyle = .inline }
        let more = NSButton(
            image: NSImage(
                systemSymbolName: "ellipsis.circle", accessibilityDescription: "More options")!,
            target: self, action: #selector(options))
        more.isBordered = false
        let footer = NSStackView(views: [pause, clear, NSView(), more])
        footer.orientation = .horizontal
        let stack = NSStackView(views: [heading, search, status, scroll, footer, hint])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 9
        stack.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 14),
            stack.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -14),
            stack.topAnchor.constraint(equalTo: content.topAnchor, constant: 12),
            stack.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -12),
            heading.widthAnchor.constraint(equalTo: stack.widthAnchor),
            search.widthAnchor.constraint(equalTo: stack.widthAnchor),
            scroll.widthAnchor.constraint(equalTo: stack.widthAnchor),
            scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 180),
            footer.widthAnchor.constraint(equalTo: stack.widthAnchor),
            hint.widthAnchor.constraint(equalTo: stack.widthAnchor),
        ])
    }
    private func registerShortcut() {
        guard !demo else { return }
        var spec = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        InstallEventHandler(
            GetApplicationEventTarget(),
            { _, _, data in
                guard let data else { return noErr }
                let delegate = Unmanaged<AppDelegate>.fromOpaque(data).takeUnretainedValue()
                DispatchQueue.main.async { delegate.toggle() }
                return noErr
            }, 1, &spec, Unmanaged.passUnretained(self).toOpaque(), &eventHandler)
        RegisterEventHotKey(
            9, UInt32(cmdKey | shiftKey), EventHotKeyID(signature: 0x4643_4C50, id: 1),
            GetApplicationEventTarget(), 0, &hotkey)
    }
    @objc func showNotification() { show() }
    @objc func toggle() { panel.isVisible ? dismiss() : show() }
    func show() {
        generation += 1
        let current = NSWorkspace.shared.frontmostApplication
        target = current?.processIdentifier != getpid() ? current : previous
        let screen =
            NSScreen.screens.first(where: { NSMouseInRect(NSEvent.mouseLocation, $0.frame, false) })
            ?? NSScreen.main
        if let frame = screen?.visibleFrame {
            panel.setFrameOrigin(
                .init(x: frame.midX - panel.frame.width / 2, y: frame.midY - panel.frame.height / 2)
            )
        }
        search.stringValue = ""
        reload()
        panel.makeKeyAndOrderFront(nil)
        panel.makeFirstResponder(search)
        service.refresh()
        updateHint()
    }
    @objc func dismiss() { panel.orderOut(nil) }
    func windowDidResignKey(_ notification: Notification) {
        if !demo && !keepOpen && !menuOpen { dismiss() }
    }
    func apply(_ snapshot: Snapshot) {
        clips = snapshot.items
        paused = snapshot.paused
        thumbnails = thumbnails.filter { key, _ in clips.contains(where: { $0.id == key }) }
        pause.title = paused ? "Resume" : "Pause"
        clear.isEnabled = !clips.isEmpty
        status.stringValue =
            snapshot.error ?? "\(clips.count) / 30 clips · \(paused ? "Paused" : "Listening")"
        reload()
        updateHint()
    }
    func reload() {
        let query = search.stringValue
        visible = clips.filter { query.isEmpty || $0.label.localizedCaseInsensitiveContains(query) }
        table.reloadData()
    }
    func updateHint() {
        hint.stringValue =
            AXIsProcessTrusted()
            ? "Click to paste · Shift-click to copy" : "Click to copy · Enable direct paste in •••"
    }
    func numberOfRows(in tableView: NSTableView) -> Int { visible.count }
    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int)
        -> NSView?
    {
        guard visible.indices.contains(row) else { return nil }
        let clip = visible[row]
        let text = NSTextField(wrappingLabelWithString: String(clip.label.prefix(400)))
        text.font = .systemFont(ofSize: 13)
        text.maximumNumberOfLines = 2
        text.lineBreakMode = .byTruncatingTail
        var views: [NSView] = []
        let image: NSImage?
        switch clip.content {
        case .text: image = nil
        case .files:
            image = NSImage(systemSymbolName: "doc.on.doc", accessibilityDescription: "Files")
        case .image(let data, _):
            if let cached = thumbnails[clip.id] {
                image = cached
            } else {
                let source = CGImageSourceCreateWithData(
                    data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary)
                let thumbnail = source.flatMap {
                    CGImageSourceCreateThumbnailAtIndex(
                        $0, 0,
                        [
                            kCGImageSourceCreateThumbnailFromImageAlways: true,
                            kCGImageSourceThumbnailMaxPixelSize: 96,
                            kCGImageSourceShouldCacheImmediately: true,
                        ] as CFDictionary)
                }
                image = thumbnail.map { NSImage(cgImage: $0, size: .zero) }
                if let image { thumbnails[clip.id] = image }
            }
        }
        if let image {
            let view = NSImageView(image: image)
            view.imageScaling = .scaleProportionallyUpOrDown
            view.widthAnchor.constraint(equalToConstant: 44).isActive = true
            view.heightAnchor.constraint(equalToConstant: 36).isActive = true
            views.append(view)
        }
        views.append(text)
        let stack = NSStackView(views: views)
        stack.orientation = .horizontal
        stack.spacing = 10
        let cell = NSTableCellView()
        cell.addSubview(stack)
        stack.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 6),
            stack.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -6),
            stack.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
        ])
        cell.setAccessibilityLabel(clip.label)
        return cell
    }
    func controlTextDidChange(_ obj: Notification) { reload() }
    func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool
    {
        if selector == #selector(NSResponder.moveDown(_:))
            || selector == #selector(NSResponder.moveUp(_:))
        {
            guard !visible.isEmpty else { return true }
            let next = selector == #selector(NSResponder.moveDown(_:)) ? 0 : visible.count - 1
            table.selectRowIndexes(.init(integer: next), byExtendingSelection: false)
            panel.makeFirstResponder(table)
            return true
        }
        if selector == #selector(NSResponder.insertNewline(_:)) {
            choose(copyOnly: NSEvent.modifierFlags.contains(.shift))
            return true
        }
        if selector == #selector(NSResponder.cancelOperation(_:)) {
            dismiss()
            return true
        }
        return false
    }
    @objc func rowClicked() { choose(copyOnly: NSEvent.modifierFlags.contains(.shift)) }
    func choose(copyOnly: Bool) {
        let row = table.selectedRow >= 0 ? table.selectedRow : 0
        guard !busy, visible.indices.contains(row) else { return }
        let clip = visible[row]
        let session = generation
        let destination = target
        busy = true
        service.copy(clip.id) { [weak self] result in
            guard let self else { return }
            self.busy = false
            if case .failure(let error) = result {
                self.hint.stringValue = error.localizedDescription
                return
            }
            if self.demo || copyOnly {
                self.hint.stringValue = "Copied"
                return
            }
            guard self.generation == session else { return }
            guard AXIsProcessTrusted() else {
                self.hint.stringValue = "Copied · Enable direct paste in •••"
                return
            }
            guard let destination, !destination.isTerminated else {
                self.hint.stringValue = "Copied · Reopen from your destination app"
                return
            }
            self.dismiss()
            destination.activate(options: [])
            self.paste(into: destination, attempt: 0)
        }
    }
    func paste(into destination: NSRunningApplication, attempt: Int) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.04) { [weak self] in
            guard let self else { return }
            guard !destination.isTerminated else { return }
            if NSWorkspace.shared.frontmostApplication?.processIdentifier
                != destination.processIdentifier
            {
                if attempt < 15 { self.paste(into: destination, attempt: attempt + 1) }
                return
            }
            let source = CGEventSource(stateID: .combinedSessionState)
            for (key, down, flags) in [
                (CGKeyCode(55), true, CGEventFlags.maskCommand), (9, true, .maskCommand),
                (9, false, .maskCommand), (55, false, CGEventFlags()),
            ] {
                let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down)
                event?.flags = flags
                event?.post(tap: .cghidEventTap)
            }
        }
    }
    @objc func togglePause() { service.togglePause() }
    @objc func clearHistory() { service.clear() }
    @objc func options(_ sender: NSButton) {
        let menu = NSMenu()
        menu.delegate = self
        menu.autoenablesItems = false
        let permission = NSMenuItem(
            title: AXIsProcessTrusted() ? "Direct paste enabled" : "Enable direct paste…",
            action: #selector(requestPaste), keyEquivalent: "")
        permission.target = self
        permission.isEnabled = !demo && !AXIsProcessTrusted()
        menu.addItem(permission)
        let login = NSMenuItem(
            title: "Start at login", action: #selector(toggleLogin), keyEquivalent: "")
        login.target = self
        login.isEnabled = !demo
        login.state = SMAppService.mainApp.status == .enabled ? .on : .off
        menu.addItem(login)
        menu.addItem(.separator())
        let quit = NSMenuItem(
            title: "Quit FeatherClip", action: #selector(quit), keyEquivalent: "q")
        quit.target = self
        menu.addItem(quit)
        menu.popUp(positioning: nil, at: .init(x: 0, y: sender.bounds.height), in: sender)
    }
    func menuWillOpen(_ menu: NSMenu) { menuOpen = true }
    func menuDidClose(_ menu: NSMenu) { menuOpen = false }
    @objc func requestPaste() {
        _ = AXIsProcessTrustedWithOptions(
            [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary)
        _ = CGRequestPostEventAccess()
        NSWorkspace.shared.open(
            URL(
                string:
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")!
        )
    }
    @objc func toggleLogin() {
        do {
            if SMAppService.mainApp.status == .enabled {
                try SMAppService.mainApp.unregister()
            } else {
                try SMAppService.mainApp.register()
            }
        } catch {
            hint.stringValue = "Couldn't update login item. Check Login Items in System Settings."
        }
    }
    @objc func quit() { NSApp.terminate(nil) }
    func smokeStep(_ timer: Timer) {
        if smokePhase == 0 && clips.count == 5 {
            guard panel.isVisible, table.numberOfRows == 5, panel.tabbingMode == .disallowed else {
                exit(1)
            }
            if #available(macOS 26.0, *) { guard nativeGlass else { exit(1) } }
            search.stringValue = "سلام"
            reload()
            guard visible.count == 1 else { exit(1) }
            search.stringValue = ""
            reload()
            choose(copyOnly: true)
            smokePhase = 1
            service.togglePause()
        } else if smokePhase == 1 && paused {
            smokePhase = 2
            service.clear()
        } else if smokePhase == 2 && clips.isEmpty {
            dismiss()
            show()
            guard panel.isVisible, visible.isEmpty else { exit(1) }
            timer.invalidate()
            print("SMOKE PASS: native AppKit, system material, search, copy, pause, clear, popup")
            NSApp.terminate(nil)
        }
    }
}

@main struct FeatherClipMain {
    @MainActor static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
        withExtendedLifetime(delegate) {}
    }
}
