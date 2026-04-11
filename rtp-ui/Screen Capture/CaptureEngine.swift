//
//  CaptureEngine.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 3/28/26.
//
import Foundation
import ScreenCaptureKit


class CaptureEngine: NSObject {
    
    private(set) var stream: SCStream?
    private let videoSampleBufferQueue = DispatchQueue(label: "com.example.apple-samplecode.VideoSampleBufferQueue")
    private let audioSampleBufferQueue = DispatchQueue(label: "com.example.apple-samplecode.AudioSampleBufferQueue")
    private let micSampleBufferQueue = DispatchQueue(label: "com.example.apple-samplecode.MicSampleBufferQueue")
    
    func startCapture(configuration: SCStreamConfiguration, filter: SCContentFilter) {
        // The stream output object. Avoid reassigning it to a new object every time startCapture is called.

        do {
            stream = SCStream(filter: filter, configuration: configuration, delegate: self)
            
            // Add a stream output to capture screen content.
            try stream?.addStreamOutput(self, type: .screen, sampleHandlerQueue: videoSampleBufferQueue)
            try stream?.addStreamOutput(self, type: .audio, sampleHandlerQueue: audioSampleBufferQueue)
            try stream?.addStreamOutput(self, type: .microphone, sampleHandlerQueue: micSampleBufferQueue)
            stream?.startCapture()
        } catch {
            fatalError("Failed to start stream: \(error)")
        }
    }
    
    func stopCapture() async {
        do {
            try await stream?.stopCapture()
        } catch {
           fatalError("Failed to stop stream: \(error)")
        }
    }
    
    func update(configuration: SCStreamConfiguration, filter: SCContentFilter) async {
        do {
            try await stream?.updateConfiguration(configuration)
            try await stream?.updateContentFilter(filter)
        } catch {
            fatalError("Failed to update the stream session: \(String(describing: error))")
        }
    }
    
    func addRecordOutputToStream(_ recordingOutput: SCRecordingOutput) async throws {
        try self.stream?.addRecordingOutput(recordingOutput)
    }
    
    func stopRecordingOutputForStream(_ recordingOutput: SCRecordingOutput) throws {
        try self.stream?.removeRecordingOutput(recordingOutput)
    }
}

/// A class that handles output from an SCStream, and handles stream errors.
extension CaptureEngine: SCStreamOutput, SCStreamDelegate {
        
    func stream(_
                stream: SCStream,
                didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
                of outputType: SCStreamOutputType
    ) {
        // Return early if the sample buffer is invalid.
        guard sampleBuffer.isValid else { return }
        

        // Determine which type of data the sample buffer contains.
//        switch outputType {
//        case .screen:
//            // TODO: Something here!
//        case .audio:
//            // TODO: And something here!
//        case .microphone:
//            // TODO: And also something here!
//        @unknown default:
//            fatalError("Encountered unknown stream output type: \(outputType)")
//        }
    }
    
    func stream(_ stream: SCStream, didStopWithError error: Error) {
        // Perhaps something here?
    }
}

