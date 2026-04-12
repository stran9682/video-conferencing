//
//  StreamManager.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 4/6/26.
//

import Foundation
import AVFoundation

class HlsWriter: NSObject {
    
    public var videoInput: AVAssetWriterInput
    public var audioInput: AVAssetWriterInput
    public var micInput: AVAssetWriterInput
    
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
        assetWriter.preferredOutputSegmentInterval = CMTime(seconds: 1, preferredTimescale: 1)
        assetWriter.initialSegmentStartTime = self.startTimeOffset
        
        
        let videoInput = AVAssetWriterInput(mediaType: .video, outputSettings: self.videoCompressionSettings)
        videoInput.expectsMediaDataInRealTime = true
        
        let audioInput = AVAssetWriterInput(mediaType: .audio, outputSettings: self.audioCompressionSettings)
        audioInput.expectsMediaDataInRealTime = true
        
        let micInput = AVAssetWriterInput(mediaType: .audio, outputSettings: self.audioCompressionSettings)
        micInput.expectsMediaDataInRealTime = true
        
        
        assetWriter.add(audioInput)
        assetWriter.add(videoInput)
        assetWriter.add(micInput)
        
        self.audioInput = audioInput
        self.videoInput = videoInput
        self.assetWriter = assetWriter
        self.micInput = micInput
        
        super.init()
        
        self.assetWriter.delegate = self
    }
    
    func startWriting() {
        assetWriter.startWriting()
        assetWriter.startSession(atSourceTime: self.startTimeOffset)
    }
}

extension HlsWriter: AVAssetWriterDelegate {
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
