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
    
    private let documentsPath: String? = NSSearchPathForDirectoriesInDomains(.documentDirectory, .userDomainMask, true).last
    
    func startCapture(configuration: SCStreamConfiguration, filter: SCContentFilter) async {
        guard stream == nil,
              let documentsPath = documentsPath
        else { return }
        
        do {
            stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
            
            let recordingsPath = documentsPath.appending("/Recordings")
            
            do {
                try FileManager.default.createDirectory(atPath: recordingsPath, withIntermediateDirectories: true, attributes: nil)
            }
            
            let outputPath = "\(recordingsPath)/\(UUID()).mp4"
            let outputURL = URL(fileURLWithPath: outputPath)
            
            let recordingConfig = SCRecordingOutputConfiguration()
            recordingConfig.outputURL = outputURL
            
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
