#!/usr/bin/env swift
//
// RecursorMenuBar - A lightweight menu bar status indicator for Recursor
//
// This runs as a background process and shows the current Recursor status
// in the macOS menu bar.
//

import Cocoa

class RecursorMenuBar: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem!
    var timer: Timer?
    let statusFile = NSHomeDirectory() + "/.cursor/recursor_status.json"
    let configFile = NSHomeDirectory() + "/.cursor/recursor_config.json"
    let iconFileName = "recursoricon.png"
    var menuBarIcon: NSImage?
    
    // Track enabled state
    var isEnabled: Bool = true
    
    func applicationDidFinishLaunching(_ notification: Notification) {
        // Load initial enabled state
        loadEnabledState()
        
        // Create status bar item
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        
        if let button = statusItem.button {
            applyStatusIcon(
                tintColor: nil,
                fallbackSymbol: "arrow.triangle.2.circlepath",
                accessibilityDescription: "Recursor",
                button: button
            )
        }
        
        // Create menu
        let menu = NSMenu()
        
        // Title with enabled/disabled indicator
        let titleItem = NSMenuItem(title: "Recursor", action: nil, keyEquivalent: "")
        titleItem.tag = 100
        menu.addItem(titleItem)
        
        menu.addItem(NSMenuItem.separator())
        
        // Toggle enabled/disabled
        let toggleItem = NSMenuItem(title: "Enabled", action: #selector(toggleEnabled), keyEquivalent: "e")
        toggleItem.tag = 10
        toggleItem.target = self
        toggleItem.state = isEnabled ? .on : .off
        menu.addItem(toggleItem)
        
        menu.addItem(NSMenuItem.separator())
        
        // Cursor state (descriptive one-liner)
        let cursorStateItem = NSMenuItem(title: "Cursor: Idle", action: nil, keyEquivalent: "")
        cursorStateItem.tag = 1
        menu.addItem(cursorStateItem)
        
        // Secondary app info
        let secondaryAppItem = NSMenuItem(title: "Secondary: None", action: nil, keyEquivalent: "")
        secondaryAppItem.tag = 2
        menu.addItem(secondaryAppItem)
        
        // Media playback status
        let mediaItem = NSMenuItem(title: "Media: -", action: nil, keyEquivalent: "")
        mediaItem.tag = 3
        menu.addItem(mediaItem)
        
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Quit", action: #selector(quit), keyEquivalent: "q"))
        
        statusItem.menu = menu
        
        // Start polling for status updates
        timer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            self?.updateStatus()
        }
        
        updateStatus()
    }
    
    func loadEnabledState() {
        guard let data = try? Data(contentsOf: URL(fileURLWithPath: configFile)),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            // Default to enabled if no config file
            isEnabled = true
            return
        }
        isEnabled = json["enabled"] as? Bool ?? true
    }
    
    func saveEnabledState() {
        let json: [String: Any] = ["enabled": isEnabled]
        if let data = try? JSONSerialization.data(withJSONObject: json, options: .prettyPrinted) {
            try? data.write(to: URL(fileURLWithPath: configFile))
        }
    }
    
    @objc func toggleEnabled() {
        isEnabled = !isEnabled
        saveEnabledState()
        
        // Update toggle menu item
        if let menu = statusItem.menu, let toggleItem = menu.item(withTag: 10) {
            toggleItem.state = isEnabled ? .on : .off
        }
        
        // Update icon appearance
        updateIconForEnabledState()
    }
    
    func updateIconForEnabledState() {
        if !isEnabled {
            applyStatusIcon(
                tintColor: .systemGray,
                fallbackSymbol: "arrow.triangle.2.circlepath",
                accessibilityDescription: "Recursor Disabled"
            )
        } else {
            updateStatus()
        }
    }
    
    func updateStatus() {
        // Reload enabled state in case it was changed externally
        loadEnabledState()
        
        // Update toggle menu item state
        if let menu = statusItem.menu, let toggleItem = menu.item(withTag: 10) {
            toggleItem.state = isEnabled ? .on : .off
        }
        
        if !isEnabled {
            setStatusDisabled()
            return
        }
        
        guard let data = try? Data(contentsOf: URL(fileURLWithPath: statusFile)),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            setStatusIdle()
            return
        }
        
        let status = json["status"] as? String ?? "idle"
        let cursorState = json["cursor_state"] as? String
        let secondaryApp = json["secondary_app"] as? String
        let secondaryTitle = json["secondary_title"] as? String
        let mediaPlaying = json["media_playing"] as? Bool
        
        setStatus(
            status: status,
            cursorState: cursorState,
            secondaryApp: secondaryApp,
            secondaryTitle: secondaryTitle,
            mediaPlaying: mediaPlaying
        )
    }
    
    func setStatusDisabled() {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            
            // Gray icon for disabled
            self.applyStatusIcon(
                tintColor: .systemGray,
                fallbackSymbol: "arrow.triangle.2.circlepath",
                accessibilityDescription: "Recursor Disabled"
            )
            
            // Update menu items
            if let menu = self.statusItem.menu {
                if let titleItem = menu.item(withTag: 100) {
                    titleItem.title = "Recursor (Disabled)"
                }
                if let cursorItem = menu.item(withTag: 1) {
                    cursorItem.title = "Cursor: Recursor disabled"
                }
                if let secondaryItem = menu.item(withTag: 2) {
                    secondaryItem.isHidden = true
                }
                if let mediaItem = menu.item(withTag: 3) {
                    mediaItem.isHidden = true
                }
            }
        }
    }
    
    func setStatusIdle() {
        setStatus(status: "idle", cursorState: nil, secondaryApp: nil, secondaryTitle: nil, mediaPlaying: nil)
    }
    
    func setStatus(status: String, cursorState: String?, secondaryApp: String?, secondaryTitle: String?, mediaPlaying: Bool?) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            
            // Update icon based on status
            switch status {
            case "working":
                self.applyStatusIcon(
                    tintColor: .systemBlue,
                    fallbackSymbol: "arrow.triangle.2.circlepath.circle.fill",
                    accessibilityDescription: "Working"
                )
            case "approval_needed":
                self.applyStatusIcon(
                    tintColor: .systemOrange,
                    fallbackSymbol: "exclamationmark.triangle.fill",
                    accessibilityDescription: "Approval Needed"
                )
            default: // idle
                self.applyStatusIcon(
                    tintColor: nil,
                    fallbackSymbol: "arrow.triangle.2.circlepath",
                    accessibilityDescription: "Idle"
                )
            }
            
            // Update menu items
            if let menu = self.statusItem.menu {
                // Title
                if let titleItem = menu.item(withTag: 100) {
                    titleItem.title = "Recursor"
                }
                
                // Cursor state
                if let cursorItem = menu.item(withTag: 1) {
                    if let state = cursorState, !state.isEmpty {
                        cursorItem.title = "Cursor: \(state)"
                    } else {
                        let defaultState: String
                        switch status {
                        case "working": defaultState = "Agent working..."
                        case "approval_needed": defaultState = "Waiting for approval..."
                        default: defaultState = "Idle"
                        }
                        cursorItem.title = "Cursor: \(defaultState)"
                    }
                }
                
                // Secondary app
                if let secondaryItem = menu.item(withTag: 2) {
                    if let app = secondaryApp, !app.isEmpty {
                        let title = secondaryTitle ?? ""
                        let truncatedTitle = title.count > 30 ? String(title.prefix(27)) + "..." : title
                        if !truncatedTitle.isEmpty {
                            secondaryItem.title = "\(app): \(truncatedTitle)"
                        } else {
                            secondaryItem.title = "Secondary: \(app)"
                        }
                        secondaryItem.isHidden = false
                    } else {
                        secondaryItem.isHidden = true
                    }
                }
                
                // Media playback
                if let mediaItem = menu.item(withTag: 3) {
                    if let playing = mediaPlaying {
                        mediaItem.title = playing ? "Media: ▶ Playing" : "Media: ⏸ Paused"
                        mediaItem.isHidden = false
                    } else {
                        mediaItem.isHidden = true
                    }
                }
            }
        }
    }
    
    @objc func quit() {
        NSApplication.shared.terminate(nil)
    }
    
    func applyStatusIcon(tintColor: NSColor?, fallbackSymbol: String, accessibilityDescription: String, button: NSStatusBarButton? = nil) {
        guard let targetButton = button ?? statusItem.button else { return }
        
        if menuBarIcon == nil {
            menuBarIcon = loadCustomMenuBarIcon()
        }
        
        if let icon = menuBarIcon {
            targetButton.image = icon
            targetButton.image?.isTemplate = true
            targetButton.contentTintColor = tintColor
            return
        }
        
        targetButton.image = NSImage(systemSymbolName: fallbackSymbol, accessibilityDescription: accessibilityDescription)
        targetButton.image?.isTemplate = true
        targetButton.contentTintColor = tintColor
    }
    
    func loadCustomMenuBarIcon() -> NSImage? {
        let fileManager = FileManager.default
        
        for url in iconSearchPaths() {
            if fileManager.fileExists(atPath: url.path),
               let rawImage = NSImage(contentsOf: url),
               let prepared = prepareMenuBarImage(rawImage) {
                return prepared
            }
        }
        
        return nil
    }
    
    func iconSearchPaths() -> [URL] {
        var candidates: [URL] = []
        
        let executableURL = URL(fileURLWithPath: CommandLine.arguments[0]).resolvingSymlinksInPath()
        let executableDir = executableURL.deletingLastPathComponent()
        let homeDir = URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)
        let currentDir = URL(fileURLWithPath: FileManager.default.currentDirectoryPath, isDirectory: true)
        
        candidates.append(executableDir.appendingPathComponent(iconFileName))
        candidates.append(homeDir.appendingPathComponent(".cursor").appendingPathComponent("bin").appendingPathComponent(iconFileName))
        candidates.append(homeDir.appendingPathComponent(".cursor").appendingPathComponent(iconFileName))
        candidates.append(currentDir.appendingPathComponent(iconFileName))
        
        if let resourceURL = Bundle.main.resourceURL {
            candidates.append(resourceURL.appendingPathComponent(iconFileName))
        }
        
        var seen: Set<String> = []
        return candidates.filter { seen.insert($0.path).inserted }
    }
    
    func prepareMenuBarImage(_ image: NSImage) -> NSImage? {
        let targetSize = NSSize(width: 18, height: 18)
        guard image.size.width > 0, image.size.height > 0 else { return nil }
        
        let scale = min(targetSize.width / image.size.width, targetSize.height / image.size.height)
        let drawSize = NSSize(width: image.size.width * scale, height: image.size.height * scale)
        let drawRect = NSRect(
            x: (targetSize.width - drawSize.width) / 2,
            y: (targetSize.height - drawSize.height) / 2,
            width: drawSize.width,
            height: drawSize.height
        )
        
        let composed = NSImage(size: targetSize)
        composed.lockFocus()
        NSGraphicsContext.current?.imageInterpolation = .high
        image.draw(in: drawRect, from: .zero, operation: .sourceOver, fraction: 1.0)
        composed.unlockFocus()
        composed.isTemplate = true
        
        return composed
    }
}

// Main entry point
let app = NSApplication.shared
let delegate = RecursorMenuBar()
app.delegate = delegate
app.setActivationPolicy(.accessory) // Hide from dock
app.run()
