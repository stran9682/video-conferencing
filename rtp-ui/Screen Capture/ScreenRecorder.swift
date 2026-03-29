//
//  ScreenRecorder.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 3/28/26.
//
import Foundation
import ScreenCaptureKit
import Combine
import OSLog

@Observable
class ScreenRecorder: NSObject {
    /// The supported capture types.
    enum CaptureType {
        case display
        case window
    }
    
    private let logger = Logger()
    
    var isRunning = false
    
    // MARK: - Video Properties
    var captureType: CaptureType = .display {
        didSet { updateEngine() }
    }
    
    var selectedDisplay: SCDisplay? {
        didSet { updateEngine() }
    }
    
    var selectedWindow: SCWindow? {
        didSet { updateEngine() }
    }
    
    var isAppExcluded = true {
        didSet { updateEngine() }
    }

    // MARK: - SCContentSharingPicker Properties
    var contentSize = CGSize(width: 1, height: 1)
    private var scaleFactor: Int { Int(NSScreen.main?.backingScaleFactor ?? 2) }
    
    /// A view that renders the screen content.
//    lazy var capturePreview: CapturePreview = {
//        CapturePreview()
//    }()
    
    private var availableApps = [SCRunningApplication]()
    private(set) var availableDisplays = [SCDisplay]()
    private(set) var availableWindows = [SCWindow]()

    // MARK: - Audio Properties
    var isAudioCaptureEnabled = true {
        didSet {
            updateEngine()
        }
    }
    var microphoneId: String?
    var isMicCaptureEnabled = false {
        didSet {
            updateEngine()
        }
    }
    var isRecordingStream = false {
        didSet {
            if isRecordingStream {
                try? initRecordingOutput()
                Task {
                    try await startRecordingOutput()
                }
            } else {
                try? stopRecordingOutput()
            }
        }
    }
    var isAppAudioExcluded = false { didSet { updateEngine() } }
    
    private let recordingOutputPath: String? = NSSearchPathForDirectoriesInDomains(.documentDirectory, .userDomainMask, true).last
    private var recordingOutput: SCRecordingOutput?
    
    // The object that manages the SCStream.
    private let captureEngine = CaptureEngine()
    
    private var isSetup = false
    
    // Combine subscribers.
    private var subscriptions = Set<AnyCancellable>()
    
    var canRecord: Bool {
        get async {
            do {
                // If the app doesn't have screen recording permission, this call generates an exception.
                try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
                return true
            } catch {
                return false
            }
        }
    }
    
    func monitorAvailableContent() async {
        guard !isSetup else { return }
        // Refresh the lists of capturable content.
        await self.refreshAvailableContent()
        Timer.publish(every: 3, on: .main, in: .common).autoconnect().sink { [weak self] _ in
            guard let self = self else { return }
            Task {
                await self.refreshAvailableContent()
            }
        }
        .store(in: &subscriptions)
    }
    
    /// Starts capturing screen content.
    func start() async {
        // Exit early if already running.
        guard !isRunning else { return }
        
        if !isSetup {
            // Starting polling for available screen content.
            await monitorAvailableContent()
            isSetup = true
        }
        
        do {
            let config = streamConfiguration
            let filter = contentFilter
            // Update the running state.
            isRunning = true
            // Start the stream and await new video frames.
            for try await frame in captureEngine.startCapture(configuration: config, filter: filter) {
                //capturePreview.updateFrame(frame)
                if contentSize != frame.size {
                    // Update the content size if it changed.
                    contentSize = frame.size
                }
            }
        } catch {
            logger.log("\(error.localizedDescription)")
            // Unable to start the stream. Set the running state to false.
            isRunning = false
        }
    }
    
    /// Stops capturing screen content.
    func stop() async {
        guard isRunning else { return }
        await captureEngine.stopCapture()
        try? stopRecordingOutput()
        removeMicrophoneOutput()
        isRunning = false
    }
    
    func openRecordingFolder() {
        if let recordingOutputPath = recordingOutputPath {
            NSWorkspace.shared.selectFile(nil, inFileViewerRootedAtPath: recordingOutputPath)
        }
    }
    
    private func addMicrophoneOutput() {
        streamConfiguration.captureMicrophone = true
    }
    private func removeMicrophoneOutput() {
        streamConfiguration.captureMicrophone = false
        streamConfiguration.microphoneCaptureDeviceID = nil
    }
    
    private func initRecordingOutput() throws {
        let dateFormatter = DateFormatter()
        dateFormatter.dateFormat = "yyyy-MM-dd_HH-mm-ss"
        let currentDateTime = dateFormatter.string(from: Date())
        if let recordingOutputPath = recordingOutputPath {
            let outputPath = "\(recordingOutputPath)/recorded_output_\(currentDateTime).mp4"
            let outputURL = URL(fileURLWithPath: outputPath)
            let recordingConfiguration = SCRecordingOutputConfiguration()
            recordingConfiguration.outputURL = outputURL
            guard let recordingOutput = (SCRecordingOutput(configuration: recordingConfiguration, delegate: self) as SCRecordingOutput?)
            else {
                throw SCScreenRecordingError.failedToStartRecording("Failed to init recording output!")
            }
            logger.log("Initialized recording output with URL \(outputURL)")
            self.recordingOutput = recordingOutput
        }
    }
    
    private func startRecordingOutput() async throws {
        guard let recordingOutput = self.recordingOutput else {
            throw SCScreenRecordingError.failedToStartRecording("Recording output is empty!")
        }
        
        try? await captureEngine.addRecordOutputToStream(recordingOutput)
        logger.log("Added recording output \(String(describing: self.recordingOutput)) successfully to stream")
        recordingOutputDidStartRecording(recordingOutput)
    }
    
    private func stopRecordingOutput() throws {
        if let recordingOutput = self.recordingOutput {
            logger.log("Stopping recording output \(recordingOutput)")
            try? captureEngine.stopRecordingOutputForStream(recordingOutput)
            recordingOutputDidFinishRecording(recordingOutput)
        }
        self.recordingOutput = nil
    }
    
    private func updateEngine() {
        guard isRunning else { return }
        Task {
            let filter = contentFilter
            await captureEngine.update(configuration: streamConfiguration, filter: filter)
        }
    }

    private var contentFilter: SCContentFilter {
        var filter: SCContentFilter
        switch captureType {
        case .display:
            guard let display = selectedDisplay else { fatalError("No display selected.") }
            
            var excludedApps = [SCRunningApplication]()
            // If a user chooses to exclude the app from the stream,
            // exclude it by matching its bundle identifier.
            if isAppExcluded {
                excludedApps = availableApps.filter { app in
                    Bundle.main.bundleIdentifier == app.bundleIdentifier
                }
            }
            // Create a content filter with excluded apps.
            filter = SCContentFilter(display: display,
                                     excludingApplications: excludedApps,
                                     exceptingWindows: [])
        case .window:
            guard let window = selectedWindow else { fatalError("No window selected.") }
            // Create a content filter that includes a single window.
            filter = SCContentFilter(desktopIndependentWindow: window)
        }
        
        return filter
    }
    
    private var streamConfiguration: SCStreamConfiguration {
        
        let streamConfig = SCStreamConfiguration()
        
        // Configure audio capture.
        streamConfig.capturesAudio = isAudioCaptureEnabled
        streamConfig.excludesCurrentProcessAudio = isAppAudioExcluded
        streamConfig.captureMicrophone = isMicCaptureEnabled
        
        // Configure the display content width and height.
        if captureType == .display, let display = selectedDisplay {
            streamConfig.width = display.width * scaleFactor
            streamConfig.height = display.height * scaleFactor
        }
        
        // Configure the window content width and height.
        if captureType == .window, let window = selectedWindow {
            streamConfig.width = Int(window.frame.width) * 2
            streamConfig.height = Int(window.frame.height) * 2
        }
        
        // Set the capture interval at 60 fps.
        streamConfig.minimumFrameInterval = CMTime(value: 1, timescale: 60)
        
        // Increase the depth of the frame queue to ensure high fps at the expense of increasing
        // the memory footprint of WindowServer.
        streamConfig.queueDepth = 5
        
        return streamConfig
    }
    
    private func refreshAvailableContent() async {
        do {
            // Retrieve the available screen content to capture.
            let availableContent = try await SCShareableContent.excludingDesktopWindows(false,
                                                                                        onScreenWindowsOnly: true)
            availableDisplays = availableContent.displays
            
            let windows = filterWindows(availableContent.windows)
            if windows != availableWindows {
                availableWindows = windows
            }
            availableApps = availableContent.applications
            
            if selectedDisplay == nil {
                selectedDisplay = availableDisplays.first
            }
            if selectedWindow == nil {
                selectedWindow = availableWindows.first
            }
        } catch {
            logger.error("Failed to get the shareable content: \(error.localizedDescription)")
        }
    }
    
    private func filterWindows(_ windows: [SCWindow]) -> [SCWindow] {
        windows
        // Sort the windows by app name.
            .sorted { $0.owningApplication?.applicationName ?? "" < $1.owningApplication?.applicationName ?? "" }
        // Remove windows that don't have an associated .app bundle.
            .filter { $0.owningApplication != nil && $0.owningApplication?.applicationName != "" }
    }
}

extension SCWindow {
    var displayName: String {
        switch (owningApplication, title) {
        case (.some(let application), .some(let title)):
            return "\(application.applicationName): \(title)"
        case (.none, .some(let title)):
            return title
        case (.some(let application), .none):
            return "\(application.applicationName): \(windowID)"
        default:
            return ""
        }
    }
}

extension SCDisplay {
    var displayName: String {
        "Display: \(width) x \(height)"
    }
}

extension ScreenRecorder: SCRecordingOutputDelegate {
    // MARK: SCRecordingOutputDelegate
    @available(macOS 15.0, *)
    nonisolated func recordingOutputDidStartRecording(_ recordingOutput: SCRecordingOutput) {
        logger.log("Recording output \(recordingOutput) did start recording")
    }

    @available(macOS 15.0, *)
    nonisolated func recordingOutputDidFinishRecording(_ recordingOutput: SCRecordingOutput) {
        logger.log("Recording output \(recordingOutput) did finish recording")
    }
}

enum SCScreenRecordingError: Error {
    case failedToStartRecording(String)
}
