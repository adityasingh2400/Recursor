#!/usr/bin/env swift
//
// RecursorMenuBar - Menu bar status + double-snap agent dashboard
//

import Cocoa
import AVFoundation

// MARK: - Snap Detector

class SnapDetector {
    private var audioEngine: AVAudioEngine?
    private var lastSnapTime: Date?
    private var isListening = false

    // Double-snap temporal parameters
    private let doubleSnapMinGap: TimeInterval = 0.1
    private let doubleSnapMaxGap: TimeInterval = 1.0
    private let cooldownPeriod: TimeInterval = 1.0
    private var lastTriggerTime: Date = .distantPast

    // Adaptive threshold state
    private var ambientRMS: Float = 0.01       // Rolling estimate of ambient noise
    private let ambientSmoothing: Float = 0.98 // How slowly ambient adapts (higher = slower)
    private let spikeMultiplier: Float = 6.0   // Spike must be Nx above ambient to count

    // Attack/decay snap shape detection
    private var prevRMS: Float = 0
    private var spikeDetected = false
    private var spikeRMS: Float = 0
    private var spikeBufferCount = 0
    private let decayWindowBuffers = 3  // Check decay within this many buffers after spike

    /// If true, a single snap triggers onSingleSnap instead of tracking doubles
    var singleSnapMode = false
    var onDoubleSnap: (() -> Void)?
    var onSingleSnap: (() -> Void)?

    func start() {
        guard !isListening else { return }
        print("[SnapDetector] Starting mic listener...")
        startListening()
    }

    func stop() {
        audioEngine?.stop()
        audioEngine?.inputNode.removeTap(onBus: 0)
        audioEngine = nil
        isListening = false
        print("[SnapDetector] Stopped")
    }

    private func startListening() {
        let engine = AVAudioEngine()
        let inputNode = engine.inputNode
        let format = inputNode.outputFormat(forBus: 0)
        print("[SnapDetector] Audio format: \(format.sampleRate)Hz, \(format.channelCount)ch")
        guard format.sampleRate > 0 else {
            print("[SnapDetector] ERROR: No audio input (sampleRate=0)")
            return
        }
        inputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buf, _ in
            self?.processBuffer(buf)
        }
        do {
            try engine.start()
            audioEngine = engine
            isListening = true
            print("[SnapDetector] Listening (adaptive threshold, spikeMultiplier=\(spikeMultiplier))")
        } catch {
            print("[SnapDetector] ERROR starting engine: \(error)")
        }
    }

    private func processBuffer(_ buffer: AVAudioPCMBuffer) {
        guard let data = buffer.floatChannelData?[0] else { return }
        let n = Int(buffer.frameLength)
        guard n > 0 else { return }

        // Calculate RMS (root mean square) energy for this buffer
        var sumSq: Float = 0
        for i in 0..<n { sumSq += data[i] * data[i] }
        let rms = sqrtf(sumSq / Float(n))

        // If we're tracking a spike, check for rapid decay
        if spikeDetected {
            spikeBufferCount += 1
            if spikeBufferCount <= decayWindowBuffers {
                // Check if signal decayed to near ambient (snap characteristic)
                if rms < spikeRMS * 0.35 {
                    // Rapid decay confirmed -- this was a snap!
                    spikeDetected = false
                    registerSnap()
                }
            } else {
                // Didn't decay fast enough -- not a snap (sustained sound)
                spikeDetected = false
            }
            prevRMS = rms
            return
        }

        // Adaptive threshold: update ambient noise estimate (only when no spike)
        ambientRMS = ambientSmoothing * ambientRMS + (1.0 - ambientSmoothing) * rms
        let threshold = max(ambientRMS * spikeMultiplier, 0.02) // Floor at 0.02

        // Detect sharp attack: RMS jumped above threshold AND previous was calm
        let attackRatio = prevRMS > 0.0001 ? rms / prevRMS : rms / 0.0001
        if rms > threshold && attackRatio > 3.0 {
            // Sharp spike detected -- start tracking decay
            spikeDetected = true
            spikeRMS = rms
            spikeBufferCount = 0
        }

        prevRMS = rms
    }

    private func registerSnap() {
        let now = Date()
        guard now.timeIntervalSince(lastTriggerTime) > cooldownPeriod else { return }

        if singleSnapMode {
            lastTriggerTime = now
            lastSnapTime = nil
            DispatchQueue.main.async { [weak self] in self?.onSingleSnap?() }
            return
        }

        if let last = lastSnapTime {
            let gap = now.timeIntervalSince(last)
            if gap >= doubleSnapMinGap && gap <= doubleSnapMaxGap {
                lastSnapTime = nil
                lastTriggerTime = now
                DispatchQueue.main.async { [weak self] in self?.onDoubleSnap?() }
            } else if gap > doubleSnapMaxGap {
                lastSnapTime = now
            }
        } else {
            lastSnapTime = now
        }
    }
}

// MARK: - Agent Info

struct AgentInfo {
    let conversationId: String
    let cursorWindow: String
    let secondaryApp: String
    let secondaryTitle: String
    let status: String
    let savedAt: Date
    var runtime: TimeInterval { Date().timeIntervalSince(savedAt) }
}

func loadAgentInfo() -> [AgentInfo] {
    let stateFile = NSHomeDirectory() + "/.cursor/recursor_state.json"
    let statusFile = NSHomeDirectory() + "/.cursor/recursor_status.json"

    guard let data = try? Data(contentsOf: URL(fileURLWithPath: stateFile)),
          let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let convos = json["conversations"] as? [String: Any] else { return [] }

    var globalStatus = "idle"
    if let sData = try? Data(contentsOf: URL(fileURLWithPath: statusFile)),
       let sJson = try? JSONSerialization.jsonObject(with: sData) as? [String: Any] {
        globalStatus = sJson["status"] as? String ?? "idle"
    }

    // Chrono serializes DateTime<Utc> with fractional seconds like:
    //   "2026-02-06T11:43:10.123456789Z"
    // Swift's ISO8601DateFormatter needs .withFractionalSeconds to parse these.
    let fmtFrac = ISO8601DateFormatter()
    fmtFrac.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    let fmtPlain = ISO8601DateFormatter()
    fmtPlain.formatOptions = [.withInternetDateTime]
    // Additional fallback for chrono's "+" offset format
    let fmtFallback = DateFormatter()
    fmtFallback.dateFormat = "yyyy-MM-dd'T'HH:mm:ss.SSSSSSSSSZZZZZ"
    fmtFallback.locale = Locale(identifier: "en_US_POSIX")

    var agents: [AgentInfo] = []
    let now = Date()
    let staleThreshold: TimeInterval = 3600 // 1 hour in seconds (same as Rust code)

    for (cid, val) in convos {
        guard let c = val as? [String: Any], !cid.hasSuffix("_shell") else { continue }
        let savedAtStr = c["saved_at"] as? String ?? ""
        let savedAt = fmtFrac.date(from: savedAtStr)
            ?? fmtPlain.date(from: savedAtStr)
            ?? fmtFallback.date(from: savedAtStr)
            ?? Date()
            
        // Filter out stale entries (older than 1 hour) to match Rust behavior
        let age = now.timeIntervalSince(savedAt)
        if age >= staleThreshold {
            continue
        }
            
        let cw = c["cursor_window"] as? [String: Any]
        let title = (cw?["title"] as? String ?? "")
            .replacingOccurrences(of: " - Cursor", with: "")
            .replacingOccurrences(of: " \u{2014} Cursor", with: "")
        let sw = c["saved_window"] as? [String: Any]
        let secApp = sw?["app_name"] as? String ?? ""
        let secTitle = sw?["title"] as? String ?? ""

        agents.append(AgentInfo(
            conversationId: cid, cursorWindow: title,
            secondaryApp: secApp, secondaryTitle: secTitle,
            status: globalStatus, savedAt: savedAt))
    }
    agents.sort { $0.runtime > $1.runtime }
    return agents
}

func formatTime(_ s: TimeInterval) -> String {
    let t = max(0, Int(s))
    let h = t / 3600; let m = (t % 3600) / 60; let sec = t % 60
    if h > 0 { return String(format: "%d:%02d:%02d", h, m, sec) }
    return String(format: "%d:%02d", m, sec)
}

// MARK: - Agent Dashboard

class AgentDashboard {
    private var panel: NSPanel?
    private var refreshTimer: Timer?
    private var runtimeLabels: [NSTextField] = []
    private var agents: [AgentInfo] = []
    /// Cache of savedAt times keyed by conversationId so timers survive close/reopen
    private var savedAtCache: [String: Date] = [:]

    var isVisible: Bool { panel?.isVisible == true }

    func toggle() { if isVisible { dismiss() } else { show() } }

    func show() {
        dismiss()
        agents = loadAgentInfo()
        // Merge with cache: keep the earliest savedAt we've seen for each conversation
        for agent in agents {
            if let cached = savedAtCache[agent.conversationId] {
                // Keep the earlier time (the real start)
                if cached < agent.savedAt {
                    // Cache has the earlier time, keep it
                    continue
                } else {
                    // Agent has the earlier time, update cache
                    savedAtCache[agent.conversationId] = agent.savedAt
                }
            } else {
                // New conversation, add to cache
                savedAtCache[agent.conversationId] = agent.savedAt
            }
        }
        // Clean cache of conversations no longer active
        let activeIds = Set(agents.map { $0.conversationId })
        savedAtCache = savedAtCache.filter { activeIds.contains($0.key) }
        runtimeLabels = []

        guard let screen = NSScreen.main else { return }
        let sf = screen.visibleFrame

        let pw: CGFloat = 600
        let rowH: CGFloat = 100
        let headerH: CGFloat = 90
        let footerH: CGFloat = 44
        let emptyH: CGFloat = 140
        let contentH = agents.isEmpty
            ? headerH + emptyH + footerH
            : headerH + CGFloat(agents.count) * rowH + footerH + 12
        let ph = min(contentH, sf.height * 0.8)
        let px = sf.midX - pw / 2
        let py = sf.midY - ph / 2

        let p = NSPanel(
            contentRect: NSRect(x: px, y: py, width: pw, height: ph),
            styleMask: [.nonactivatingPanel, .titled, .closable, .fullSizeContentView],
            backing: .buffered, defer: false)
        p.isFloatingPanel = true
        p.level = .floating
        p.titlebarAppearsTransparent = true
        p.titleVisibility = .hidden
        p.isMovableByWindowBackground = true
        p.backgroundColor = NSColor(red: 0.11, green: 0.11, blue: 0.12, alpha: 0.98)
        p.isOpaque = false
        p.hasShadow = true

        let blur = NSVisualEffectView(frame: NSRect(x: 0, y: 0, width: pw, height: ph))
        blur.blendingMode = .behindWindow
        blur.material = .hudWindow
        blur.state = .active
        blur.autoresizingMask = [.width, .height]

        let cv = NSView(frame: NSRect(x: 0, y: 0, width: pw, height: ph))
        cv.addSubview(blur)

        // -- Header --
        let hy = ph - headerH

        let title = NSTextField(labelWithString: "Agent Dashboard")
        title.font = NSFont.systemFont(ofSize: 16, weight: .semibold)
        title.textColor = NSColor(red: 0.85, green: 0.85, blue: 0.87, alpha: 1)
        title.frame = NSRect(x: 24, y: hy + 40, width: 250, height: 22)
        cv.addSubview(title)

        // Subtitle
        let subtitle = NSTextField(labelWithString: "Active Cursor agent sessions")
        subtitle.font = NSFont.systemFont(ofSize: 11)
        subtitle.textColor = NSColor(white: 0.38, alpha: 1)
        subtitle.frame = NSRect(x: 24, y: hy + 20, width: 250, height: 16)
        cv.addSubview(subtitle)

        // Active count badge (pill with centered text)
        let count = agents.count
        let badgeText = count == 1 ? "1 agent" : "\(count) agents"
        let badgeH: CGFloat = 24
        let badgeW: CGFloat = CGFloat(badgeText.count) * 7.5 + 22
        let badgeBg = NSView(frame: NSRect(x: pw - badgeW - 24, y: hy + 38, width: badgeW, height: badgeH))
        badgeBg.wantsLayer = true
        badgeBg.layer?.masksToBounds = true
        badgeBg.layer?.cornerRadius = badgeH / 2
        badgeBg.layer?.backgroundColor = count > 0
            ? NSColor(red: 0.31, green: 0.79, blue: 0.69, alpha: 0.12).cgColor
            : NSColor(white: 0.18, alpha: 1).cgColor
        cv.addSubview(badgeBg)

        let badgeLabel = NSTextField(labelWithString: badgeText)
        badgeLabel.font = NSFont.systemFont(ofSize: 11, weight: .medium)
        badgeLabel.textColor = count > 0
            ? NSColor(red: 0.31, green: 0.79, blue: 0.69, alpha: 1)
            : NSColor(white: 0.45, alpha: 1)
        badgeLabel.alignment = .center
        badgeLabel.frame = NSRect(x: 0, y: (badgeH - 14) / 2, width: badgeW, height: 14)
        badgeBg.addSubview(badgeLabel)

        let sep = NSBox(); sep.boxType = .separator
        sep.frame = NSRect(x: 20, y: hy + 8, width: pw - 40, height: 1)
        cv.addSubview(sep)

        // -- Agent rows --
        if agents.isEmpty {
            let ey = hy - emptyH
            let el = NSTextField(labelWithString: "No active agents")
            el.font = NSFont.systemFont(ofSize: 15, weight: .medium)
            el.textColor = NSColor(white: 0.4, alpha: 1)
            el.alignment = .center
            el.frame = NSRect(x: 0, y: ey + 75, width: pw, height: 22)
            cv.addSubview(el)

            let hl = NSTextField(labelWithString: "Submit a prompt in Cursor to start an agent")
            hl.font = NSFont.systemFont(ofSize: 12)
            hl.textColor = NSColor(white: 0.28, alpha: 1)
            hl.alignment = .center
            hl.frame = NSRect(x: 0, y: ey + 50, width: pw, height: 18)
            cv.addSubview(hl)
        } else {
            for (i, agent) in agents.enumerated() {
                let ry = hy - CGFloat(i + 1) * rowH
                buildAgentCard(parent: cv, agent: agent, y: ry, w: pw, index: i)
            }
        }

        // -- Footer --
        let fl = NSTextField(labelWithString: "snap to dismiss  |  esc to close")
        fl.font = NSFont.systemFont(ofSize: 10)
        fl.textColor = NSColor(white: 0.3, alpha: 1)
        fl.alignment = .center
        fl.frame = NSRect(x: 0, y: 10, width: pw, height: 14)
        cv.addSubview(fl)

        p.contentView = cv
        p.makeKeyAndOrderFront(nil)
        self.panel = p

        refreshTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            self?.refreshTimers()
        }

        NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            if event.keyCode == 53 { self?.dismiss(); return nil }
            return event
        }
    }

    private func buildAgentCard(parent: NSView, agent: AgentInfo, y: CGFloat, w: CGFloat, index: Int) {
        let card = NSView(frame: NSRect(x: 16, y: y + 6, width: w - 32, height: 84))
        card.wantsLayer = true
        card.layer?.backgroundColor = NSColor(red: 0.15, green: 0.15, blue: 0.16, alpha: 1).cgColor
        card.layer?.cornerRadius = 10
        card.layer?.borderWidth = 1
        card.layer?.borderColor = NSColor(white: 0.22, alpha: 1).cgColor
        parent.addSubview(card)

        let cw = w - 32

        // Status color
        let statusColor: NSColor
        let statusText: String
        switch agent.status {
        case "working":
            statusColor = NSColor(red: 0.0, green: 0.47, blue: 0.83, alpha: 1) // Cursor blue
            statusText = "Working"
        case "approval_needed":
            statusColor = NSColor(red: 0.84, green: 0.73, blue: 0.49, alpha: 1) // Cursor warning
            statusText = "Needs Approval"
        default:
            statusColor = NSColor(white: 0.45, alpha: 1)
            statusText = "Idle"
        }

        // Status pill (left side)
        let pill = NSTextField(labelWithString: statusText.uppercased())
        pill.font = NSFont.systemFont(ofSize: 9, weight: .bold)
        pill.textColor = statusColor
        pill.alignment = .center
        pill.isBezeled = false
        pill.drawsBackground = true
        pill.backgroundColor = statusColor.withAlphaComponent(0.12)
        pill.wantsLayer = true
        pill.layer?.cornerRadius = 4
        let pillW: CGFloat = CGFloat(statusText.count) * 6.5 + 12
        pill.frame = NSRect(x: 16, y: 56, width: pillW, height: 16)
        card.addSubview(pill)

        // Project name
        let projName = agent.cursorWindow.isEmpty ? "Cursor" : agent.cursorWindow
        let projLabel = NSTextField(labelWithString: projName)
        projLabel.font = NSFont.systemFont(ofSize: 14, weight: .semibold)
        projLabel.textColor = NSColor(red: 0.85, green: 0.85, blue: 0.87, alpha: 1)
        projLabel.lineBreakMode = .byTruncatingTail
        projLabel.frame = NSRect(x: 16, y: 34, width: cw - 160, height: 20)
        card.addSubview(projLabel)

        // Descriptive status line based on what the agent is doing
        let secName = agent.secondaryApp.isEmpty ? "None" : agent.secondaryApp
        let secTitleShort = agent.secondaryTitle.count > 35
            ? String(agent.secondaryTitle.prefix(32)) + "..."
            : agent.secondaryTitle
        let detail: String
        switch agent.status {
        case "working":
            if secTitleShort.isEmpty {
                detail = "Agent is coding. You were sent to \(secName)."
            } else {
                detail = "Agent is coding. You were sent to \(secName) (\(secTitleShort))."
            }
        case "approval_needed":
            detail = "Agent needs your approval to run a command."
        default:
            if secTitleShort.isEmpty {
                detail = "Waiting in \(secName)."
            } else {
                detail = "Waiting in \(secName) (\(secTitleShort))."
            }
        }
        let detailLabel = NSTextField(labelWithString: detail)
        detailLabel.font = NSFont.systemFont(ofSize: 11)
        detailLabel.textColor = NSColor(white: 0.45, alpha: 1)
        detailLabel.lineBreakMode = .byTruncatingTail
        detailLabel.frame = NSRect(x: 16, y: 16, width: cw - 160, height: 16)
        card.addSubview(detailLabel)

        // Runtime (right side, large)
        let cachedStart = savedAtCache[agent.conversationId] ?? agent.savedAt
        let rt = Date().timeIntervalSince(cachedStart)
        let timeLabel = NSTextField(labelWithString: formatTime(rt))
        timeLabel.font = NSFont.monospacedDigitSystemFont(ofSize: 24, weight: .medium)
        timeLabel.textColor = statusColor
        timeLabel.alignment = .right
        timeLabel.frame = NSRect(x: cw - 148, y: 38, width: 130, height: 30)
        card.addSubview(timeLabel)
        runtimeLabels.append(timeLabel)

        // "elapsed" label under timer
        let elLabel = NSTextField(labelWithString: "elapsed")
        elLabel.font = NSFont.systemFont(ofSize: 9, weight: .regular)
        elLabel.textColor = NSColor(white: 0.35, alpha: 1)
        elLabel.alignment = .right
        elLabel.frame = NSRect(x: cw - 148, y: 26, width: 130, height: 12)
        card.addSubview(elLabel)
    }

    private var refreshCount = 0

    private func refreshTimers() {
        refreshCount += 1

        // Every 3 seconds, re-check the state file for changes
        if refreshCount % 3 == 0 {
            let freshAgents = loadAgentInfo()
            let freshIds = Set(freshAgents.map { $0.conversationId })
            let staleIds = agents.filter { !freshIds.contains($0.conversationId) }

            if !staleIds.isEmpty {
                // An agent finished -- remove from cache and rebuild the dashboard
                for stale in staleIds {
                    savedAtCache.removeValue(forKey: stale.conversationId)
                }
                // Rebuild the entire panel with fresh data
                show()
                return
            }
        }

        // Update timer labels
        for (i, label) in runtimeLabels.enumerated() {
            guard i < agents.count else { break }
            let start = savedAtCache[agents[i].conversationId] ?? agents[i].savedAt
            label.stringValue = formatTime(Date().timeIntervalSince(start))
        }
    }

    func dismiss() {
        refreshTimer?.invalidate()
        refreshTimer = nil
        runtimeLabels = []
        panel?.orderOut(nil)
        panel = nil
    }
}

// MARK: - Menu Bar App

class RecursorMenuBar: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem!
    var timer: Timer?
    let statusFile = NSHomeDirectory() + "/.cursor/recursor_status.json"
    let configFile = NSHomeDirectory() + "/.cursor/recursor_config.json"
    var isEnabled: Bool = true
    let snapDetector = SnapDetector()
    let dashboard = AgentDashboard()

    func loadCustomIcon() -> NSImage? {
        let paths = [
            NSHomeDirectory() + "/.cursor/bin/recursor-icon@2x.png",
            NSHomeDirectory() + "/.cursor/bin/recursor-icon.png",
            (CommandLine.arguments.first.flatMap {
                URL(fileURLWithPath: $0).deletingLastPathComponent().path
            } ?? ".") + "/../menubar/icon@2x.png",
        ]
        for p in paths {
            if let img = NSImage(contentsOfFile: p) {
                img.size = NSSize(width: 22, height: 22)
                img.isTemplate = false
                return img
            }
        }
        return nil
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        loadEnabledState()
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let b = statusItem.button {
            if let ic = loadCustomIcon() { b.image = ic }
            else {
                b.image = NSImage(systemSymbolName: "arrow.triangle.2.circlepath",
                                  accessibilityDescription: "Recursor")
                b.image?.isTemplate = true
            }
        }

        let menu = NSMenu()
        let ti = NSMenuItem(title: "Recursor", action: nil, keyEquivalent: "")
        ti.tag = 100; menu.addItem(ti)
        menu.addItem(.separator())

        let tog = NSMenuItem(title: "Enabled", action: #selector(toggleEnabled), keyEquivalent: "e")
        tog.tag = 10; tog.target = self; tog.state = isEnabled ? .on : .off
        menu.addItem(tog)
        menu.addItem(.separator())

        let dash = NSMenuItem(title: "Agent Dashboard", action: #selector(showDash), keyEquivalent: "d")
        dash.target = self; menu.addItem(dash)

        let snap = NSMenuItem(title: "Snap Detection", action: #selector(toggleSnap), keyEquivalent: "s")
        snap.tag = 20; snap.target = self; snap.state = .on
        menu.addItem(snap)
        menu.addItem(.separator())

        let cs = NSMenuItem(title: "Cursor: Idle", action: nil, keyEquivalent: "")
        cs.tag = 1; menu.addItem(cs)
        let sa = NSMenuItem(title: "Secondary: None", action: nil, keyEquivalent: "")
        sa.tag = 2; menu.addItem(sa)
        let mi = NSMenuItem(title: "Media: -", action: nil, keyEquivalent: "")
        mi.tag = 3; menu.addItem(mi)

        menu.addItem(.separator())
        menu.addItem(NSMenuItem(title: "Quit", action: #selector(quit), keyEquivalent: "q"))
        statusItem.menu = menu

        timer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            self?.updateStatus()
        }
        updateStatus()

        // Wire up snap detection
        snapDetector.onDoubleSnap = { [weak self] in
            guard let self = self else { return }
            self.dashboard.show()
            // Switch to single-snap mode while dashboard is open
            self.snapDetector.singleSnapMode = true
        }
        snapDetector.onSingleSnap = { [weak self] in
            guard let self = self else { return }
            if self.dashboard.isVisible {
                self.dashboard.dismiss()
                self.snapDetector.singleSnapMode = false
            }
        }
        snapDetector.start()
    }

    @objc func showDash() { dashboard.toggle() }

    @objc func toggleSnap() {
        if let m = statusItem.menu, let s = m.item(withTag: 20) {
            if s.state == .on { snapDetector.stop(); s.state = .off }
            else { snapDetector.start(); s.state = .on }
        }
    }

    func loadEnabledState() {
        guard let d = try? Data(contentsOf: URL(fileURLWithPath: configFile)),
              let j = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else {
            isEnabled = true; return
        }
        isEnabled = j["enabled"] as? Bool ?? true
    }

    func saveEnabledState() {
        let j: [String: Any] = ["enabled": isEnabled]
        if let d = try? JSONSerialization.data(withJSONObject: j, options: .prettyPrinted) {
            try? d.write(to: URL(fileURLWithPath: configFile))
        }
    }

    @objc func toggleEnabled() {
        isEnabled = !isEnabled; saveEnabledState()
        if let m = statusItem.menu, let t = m.item(withTag: 10) { t.state = isEnabled ? .on : .off }
        updateIconForState()
    }

    func updateIconForState() {
        guard let b = statusItem.button else { return }
        if !isEnabled {
            if let ic = loadCustomIcon() { b.image = ic }
            else {
                b.image = NSImage(systemSymbolName: "arrow.triangle.2.circlepath",
                                  accessibilityDescription: "Disabled")
                b.image?.isTemplate = true
            }
            b.contentTintColor = .systemGray
        }
    }

    func updateStatus() {
        loadEnabledState()
        if let m = statusItem.menu, let t = m.item(withTag: 10) { t.state = isEnabled ? .on : .off }
        if !isEnabled { setDisabled(); return }
        guard let d = try? Data(contentsOf: URL(fileURLWithPath: statusFile)),
              let j = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else {
            setIdle(); return
        }
        setFull(
            status: j["status"] as? String ?? "idle",
            cursorState: j["cursor_state"] as? String,
            secondaryApp: j["secondary_app"] as? String,
            secondaryTitle: j["secondary_title"] as? String,
            mediaPlaying: j["media_playing"] as? Bool)
    }

    func setDisabled() {
        DispatchQueue.main.async { [weak self] in
            guard let s = self, let b = s.statusItem.button else { return }
            if let ic = s.loadCustomIcon() { b.image = ic }
            else {
                b.image = NSImage(systemSymbolName: "arrow.triangle.2.circlepath",
                                  accessibilityDescription: "Disabled")
                b.image?.isTemplate = true
            }
            b.contentTintColor = .systemGray
            if let m = s.statusItem.menu {
                m.item(withTag: 100)?.title = "Recursor (Disabled)"
                m.item(withTag: 1)?.title = "Cursor: Disabled"
                m.item(withTag: 2)?.isHidden = true
                m.item(withTag: 3)?.isHidden = true
            }
        }
    }

    func setIdle() {
        setFull(status: "idle", cursorState: nil, secondaryApp: nil,
                secondaryTitle: nil, mediaPlaying: nil)
    }

    func setFull(status: String, cursorState: String?, secondaryApp: String?,
                 secondaryTitle: String?, mediaPlaying: Bool?) {
        DispatchQueue.main.async { [weak self] in
            guard let s = self, let b = s.statusItem.button else { return }
            // Always keep the custom icon - never swap to SF Symbols
            if let ic = s.loadCustomIcon() { b.image = ic }
            else {
                b.image = NSImage(systemSymbolName: "arrow.triangle.2.circlepath",
                                  accessibilityDescription: "Recursor")
                b.image?.isTemplate = true
            }
            b.contentTintColor = nil
            if let m = s.statusItem.menu {
                m.item(withTag: 100)?.title = "Recursor"
                if let ci = m.item(withTag: 1) {
                    if let st = cursorState, !st.isEmpty { ci.title = "Cursor: \(st)" }
                    else {
                        switch status {
                        case "working": ci.title = "Cursor: Agent working..."
                        case "approval_needed": ci.title = "Cursor: Waiting for approval..."
                        default: ci.title = "Cursor: Idle"
                        }
                    }
                }
                if let si = m.item(withTag: 2) {
                    if let app = secondaryApp, !app.isEmpty {
                        let t = secondaryTitle ?? ""
                        let short = t.count > 30 ? String(t.prefix(27)) + "..." : t
                        si.title = short.isEmpty ? "Secondary: \(app)" : "\(app): \(short)"
                        si.isHidden = false
                    } else { si.isHidden = true }
                }
                if let mi = m.item(withTag: 3) {
                    if let p = mediaPlaying {
                        mi.title = p ? "Media: ▶ Playing" : "Media: ⏸ Paused"
                        mi.isHidden = false
                    } else { mi.isHidden = true }
                }
            }
        }
    }

    @objc func quit() { snapDetector.stop(); NSApplication.shared.terminate(nil) }
}

// MARK: - Main
let app = NSApplication.shared
let delegate = RecursorMenuBar()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
