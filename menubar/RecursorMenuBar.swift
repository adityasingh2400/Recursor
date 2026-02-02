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
    
    func applicationDidFinishLaunching(_ notification: Notification) {
        // Create status bar item
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        
        if let button = statusItem.button {
            button.image = NSImage(systemSymbolName: "arrow.triangle.2.circlepath", accessibilityDescription: "Recursor")
            button.image?.isTemplate = true
        }
        
        // Create menu
        let menu = NSMenu()
        menu.addItem(NSMenuItem(title: "Recursor", action: nil, keyEquivalent: ""))
        menu.addItem(NSMenuItem.separator())
        
        let statusMenuItem = NSMenuItem(title: "Status: Idle", action: nil, keyEquivalent: "")
        statusMenuItem.tag = 1
        menu.addItem(statusMenuItem)
        
        let windowMenuItem = NSMenuItem(title: "Window: None", action: nil, keyEquivalent: "")
        windowMenuItem.tag = 2
        menu.addItem(windowMenuItem)
        
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Quit", action: #selector(quit), keyEquivalent: "q"))
        
        statusItem.menu = menu
        
        // Start polling for status updates
        timer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            self?.updateStatus()
        }
        
        updateStatus()
    }
    
    func updateStatus() {
        guard let data = try? Data(contentsOf: URL(fileURLWithPath: statusFile)),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            setStatus("idle", window: nil)
            return
        }
        
        let status = json["status"] as? String ?? "idle"
        let window = json["window"] as? String
        
        setStatus(status, window: window)
    }
    
    func setStatus(_ status: String, window: String?) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self, let button = self.statusItem.button else { return }
            
            // Update icon based on status
            switch status {
            case "working":
                button.image = NSImage(systemSymbolName: "arrow.triangle.2.circlepath.circle.fill", accessibilityDescription: "Working")
                // Add a subtle animation effect by changing the tint
                button.contentTintColor = .systemBlue
            default:
                button.image = NSImage(systemSymbolName: "arrow.triangle.2.circlepath", accessibilityDescription: "Idle")
                button.contentTintColor = nil
            }
            button.image?.isTemplate = (status == "idle")
            
            // Update menu items
            if let menu = self.statusItem.menu {
                if let statusItem = menu.item(withTag: 1) {
                    statusItem.title = "Status: \(status.capitalized)"
                }
                if let windowItem = menu.item(withTag: 2) {
                    if let window = window, !window.isEmpty {
                        // Truncate long titles
                        let truncated = window.count > 40 ? String(window.prefix(37)) + "..." : window
                        windowItem.title = "Window: \(truncated)"
                        windowItem.isHidden = false
                    } else {
                        windowItem.isHidden = true
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
