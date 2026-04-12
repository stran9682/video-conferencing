//
//  StreamManager.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 4/6/26.
//

import Foundation
import AVFoundation

class HLSWriter: NSObject {
    
    public var videoInput: AVAssetWriterInput
    public var audioInput: AVAssetWriterInput
    
    private var assetWriter: AVAssetWriter
    
    private let audioCompressionSettings: [String: Any] = [
        AVFormatIDKey: kAudioFormatMPEG4AAC,
        AVSampleRateKey: 44_100,
        AVNumberOfChannelsKey: 2,
        AVEncoderBitRateKey: 160_000
    ]
    
    private let videoCompressionSettings: [String: Any] = [
        AVVideoCodecKey: AVVideoCodecType.h264,
        AVVideoWidthKey: 1280,
        AVVideoHeightKey: 720,
    ]
    
    private let startTimeOffset = CMTime(value: 1, timescale: 10) // 100 ms
    
    override init() {
        let assetWriter = AVAssetWriter(contentType: .mpeg4Movie)
        assetWriter.shouldOptimizeForNetworkUse = true
        assetWriter.outputFileTypeProfile = .mpeg4AppleHLS
        assetWriter.preferredOutputSegmentInterval = CMTime(seconds: 6, preferredTimescale: 1)
        assetWriter.initialSegmentStartTime = self.startTimeOffset
        
        let videoInput = AVAssetWriterInput(mediaType: .video, outputSettings: self.videoCompressionSettings)
        videoInput.expectsMediaDataInRealTime = true
        
        let audioInput = AVAssetWriterInput(mediaType: .audio, outputSettings: self.audioCompressionSettings)
        audioInput.expectsMediaDataInRealTime = true

        assetWriter.add(audioInput)
        assetWriter.add(videoInput)
        
        self.audioInput = audioInput
        self.videoInput = videoInput
        self.assetWriter = assetWriter
        
        super.init()
        
        self.assetWriter.delegate = self
    }
    
    func startWriting() {
        assetWriter.startWriting()
        assetWriter.startSession(atSourceTime: self.startTimeOffset)
    }
    
    func finishWriting() async {
        await assetWriter.finishWriting()
    }
    
    func appendAudio(_ buffer: CMSampleBuffer) throws {
        try self.append(buffer: buffer, input: self.audioInput)
    }
    
    func appendVideo(_ buffer: CMSampleBuffer) throws {
        try self.append(buffer: buffer, input: self.videoInput)
    }
    
    private func append(buffer: CMSampleBuffer, input: AVAssetWriterInput) throws {
        // False if the input was not ready for more media data.
        // ignore the result even if it is not added to the writer successfully due to the writer is not ready.
        // Reason: since we are getting the buffer continuously, if we try to wait to append, the time lag will keep increasing.
        guard input.isReadyForMoreMediaData else {
            return
        }
        
        // A BOOL value indicating success of appending the sample buffer. If a result of NO is returned, clients can check the value of AVAssetWriter.status to determine whether the writing operation completed, failed, or was cancelled.  If the status is AVAssetWriterStatusFailed, AVAsset.error will contain an instance of NSError that describes the failure.
        let success = input.append(buffer)
        if success {
            return
        }
        
        try checkError()
    }
    
    private func checkError() throws {
        if assetWriter.status == .failed, let error = assetWriter.error {
            throw error
        }
    }
}

extension HLSWriter: AVAssetWriterDelegate {
    func assetWriter(_ writer: AVAssetWriter,
                     didOutputSegmentData segmentData: Data,
                     segmentType: AVAssetSegmentType,
                     segmentReport: AVAssetSegmentReport?
    ) {        
        switch segmentType {
        case .initialization:
            // This is your 'init.mp4'. Save it or send it to the server first.
            print("Received Init Segment: \(segmentData.count) bytes")
            
        case .separable:
            // This is a media segment (e.g., 'segment1.m4s').
            // The segmentReport contains the duration and timing info.
            if let report = segmentReport {
                print("Received Segment: \(report.trackReports.first?.duration.seconds ?? 0)s")
            }
            // Logic to save file or update your .m3u8 playlist goes here
        @unknown default:
            return
        }
    }
}
