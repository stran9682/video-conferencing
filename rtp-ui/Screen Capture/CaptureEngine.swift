//
//  CaptureEngine.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 3/28/26.
//
import Foundation
import ScreenCaptureKit


class CaptureEngine: NSObject, SCRecordingOutputDelegate {
    
    private var stream: SCStream?
    private var recordingOutput: SCRecordingOutput?
    private var isRunning = false
    
    func startCapture(configuration: SCStreamConfiguration, filter: SCContentFilter) async {
        guard stream == nil else { return }
        
        do {
            stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
            
            let videoURL = FileManager.default.temporaryDirectory.appendingPathComponent("\(UUID()).mp4")
            let recordingConfig = SCRecordingOutputConfiguration()
            recordingConfig.outputURL = videoURL
            
            recordingOutput = SCRecordingOutput(configuration: recordingConfig, delegate: self)
            try stream?.addRecordingOutput(recordingOutput!)
                        
            try await stream?.startCapture()
        } catch {
            print("Failed to start stream: \(error)")
        }
    }
    
    func stopCapture() async {
        guard let stream = self.stream,
              let recordingOutput = self.recordingOutput
        else { return }
        
        do {
            try await stream.stopCapture()
            try stream.removeRecordingOutput(recordingOutput)
        } catch {
            print("Failed to stop stream: \(error)")
        }
        
        isRunning = false
        self.stream = nil
        self.recordingOutput = nil
    }
}
