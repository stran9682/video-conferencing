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
    
    func startCapture(configuration: SCStreamConfiguration, filter: SCContentFilter) async {
        guard stream == nil else { return }
        
        do {
            stream = SCStream(filter: filter, configuration: configuration, delegate: self)
            
            // Add a stream output to capture screen content.
            if configuration.captureMicrophone {
                try stream?.addStreamOutput(self, type: .microphone, sampleHandlerQueue: micSampleBufferQueue)
            }
            
            if configuration.capturesAudio {
                try stream?.addStreamOutput(self, type: .audio, sampleHandlerQueue: audioSampleBufferQueue)
            }
            
            try stream?.addStreamOutput(self, type: .screen, sampleHandlerQueue: videoSampleBufferQueue)
            
            try await stream?.startCapture()
        } catch {
            print("Failed to start stream: \(error)")
        }
    }
    
    func stopCapture() async {
        guard let stream = self.stream else { return }
        do {
            try await stream.stopCapture()
            self.stream = nil
        } catch {
           print("Failed to stop stream: \(error)")
            self.stream = nil
        }
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
        
        print("Recieved a sample buffer!")
        
        guard sampleBuffer.isValid else { return }
        
        print("output type is \(outputType)")
        
        // Determine which type of data the sample buffer contains.
//        switch outputType {
//        case .screen:
//            streamManager.videoInput.append(sampleBuffer)
//        case .audio:
//            streamManager.audioInput.append(sampleBuffer)
//        case .microphone:
//            streamManager.micInput.append(sampleBuffer)
//        @unknown default:
//            print("Hmm, I don't know what this is: \(outputType)")
//        }
    }
    
    func stream(_ stream: SCStream, didStopWithError error: Error) {
        // Perhaps something here?
        print(error)
    }
}

