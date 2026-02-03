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
    
    // Track enabled state
    var isEnabled: Bool = true
    
    func applicationDidFinishLaunching(_ notification: Notification) {
        // Load initial enabled state
        loadEnabledState()
        
        // Create status bar item
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        
        if let button = statusItem.button {
            button.image = NSImage(systemSymbolName: "arrow.triangle.2.circlepath", accessibilityDescription: "Recursor")
            button.image?.isTemplate = true
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
        guard let button = statusItem.button else { return }
        
        if !isEnabled {
            // Disabled: gray icon
            button.image = NSImage(systemSymbolName: "arrow.triangle.2.circlepath", accessibilityDescription: "Recursor Disabled")
            button.image?.isTemplate = true
            button.contentTintColor = .systemGray
        }
        // If enabled, updateStatus() will set the appropriate icon
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
            guard let self = self, let button = self.statusItem.button else { return }
            
            // Gray icon for disabled
            button.image = NSImage(systemSymbolName: "arrow.triangle.2.circlepath", accessibilityDescription: "Recursor Disabled")
            button.image?.isTemplate = true
            button.contentTintColor = .systemGray
            
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
            guard let self = self, let button = self.statusItem.button else { return }
            
            // Update icon based on status
            switch status {
            case "working":
                button.image = NSImage(systemSymbolName: "arrow.triangle.2.circlepath.circle.fill", accessibilityDescription: "Working")
                button.contentTintColor = .systemBlue
                button.image?.isTemplate = false
            case "approval_needed":
                button.image = NSImage(systemSymbolName: "exclamationmark.triangle.fill", accessibilityDescription: "Approval Needed")
                button.contentTintColor = .systemOrange
                button.image?.isTemplate = false
            default: // idle
                button.image = NSImage(systemSymbolName: "arrow.triangle.2.circlepath", accessibilityDescription: "Idle")
                button.contentTintColor = nil
                button.image?.isTemplate = true
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
}

// Main entry point
let app = NSApplication.shared
let delegate = RecursorMenuBar()
app.delegate = delegate
app.setActivationPolicy(.accessory) // Hide from dock
app.run()
