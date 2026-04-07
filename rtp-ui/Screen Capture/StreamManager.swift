//
//  StreamManager.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 4/6/26.
//

import Foundation
import AVFoundation

class StreamManager: NSObject {
    
    public var videoInput: AVAssetWriterInput?
    private var assetWriter: AVAssetWriter?
    
    override init() {
        assetWriter = AVAssetWriter(contentType: UTType(AVFileType.mp4.rawValue)!)
        videoInput = AVAssetWriterInput(mediaType: .video, outputSettings: nil)
        
        assetWriter?.add(videoInput!)
        
    }
    
}

extension StreamManager: AVAssetWriterDelegate {
    func assetWriter(_ writer: AVAssetWriter, didOutputSegmentData segmentData: Data, segmentType: AVAssetSegmentType) {
        
    }
}
