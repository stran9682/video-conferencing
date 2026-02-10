//
//  VideoManager.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/19/26.
//

import Foundation
import VideoToolbox

class DecompressionManager {
    var session: VTDecompressionSession?
    var formatDescription: CMFormatDescription?
        
    init (sps: UnsafePointer<UInt8>, spsLength: Int, pps: UnsafePointer<UInt8>, ppsLength: Int) {
       
        let paramSetPointers: [UnsafePointer<UInt8>] = [sps, pps]
        
        let parameterSetSizes: [Int] = [spsLength, ppsLength]
        
        CMVideoFormatDescriptionCreateFromH264ParameterSets(
            allocator: kCFAllocatorDefault,
            parameterSetCount: 2,
            parameterSetPointers: paramSetPointers,
            parameterSetSizes: parameterSetSizes,
            nalUnitHeaderLength: 4,
            formatDescriptionOut: &formatDescription
        )
        
        let decoderSpecification = [
            kVTVideoDecoderSpecification_RequireHardwareAcceleratedVideoDecoder: true as CFBoolean
        ] as CFDictionary
        
        // sets what object will be calling the callback (this one lol)
        let refcon = Unmanaged.passUnretained(self).toOpaque()
        var callbackRecord = VTDecompressionOutputCallbackRecord(
            decompressionOutputCallback: callback,
            decompressionOutputRefCon: refcon
        )
        
        if let formatDescription = formatDescription {
            VTDecompressionSessionCreate(
                allocator: kCFAllocatorDefault,
                formatDescription: formatDescription,
                decoderSpecification: decoderSpecification,
                imageBufferAttributes: nil,
                outputCallback: &callbackRecord,
                decompressionSessionOut: &session)
        }
    }
    
    
    // this is called as part of the callback from decompression.
    func processImage(_ image: CVImageBuffer, time: CMTime, duration: CMTime) {
        
        var sampleBuffer: CMSampleBuffer?
        var sampleTiming = CMSampleTimingInfo(duration: duration, presentationTimeStamp: time, decodeTimeStamp: time)

        var formatDesc: CMFormatDescription? = nil
        CMVideoFormatDescriptionCreateForImageBuffer(
            allocator: kCFAllocatorDefault,
            imageBuffer: image,
            formatDescriptionOut: &formatDesc
        )
        
        guard let formatDescription = formatDesc else {
            fatalError("formatDescription")
        }

        let status = CMSampleBufferCreateReadyWithImageBuffer(
            allocator: kCFAllocatorDefault,
            imageBuffer: image,
            formatDescription: formatDescription,
            sampleTiming: &sampleTiming,
            sampleBufferOut: &sampleBuffer)
        
        if status != noErr {
            print("CMSampleBufferCreateReadyWithImageBuffer failure \(status)")
        }
//        if let sb = sampleBuffer {
//            handler?(sb)
//        }
        
        // MARK: UPDATE THE PREVIEW STREAM, back to the main thread!
    }
    
    func decode (sampleBuffer: CMSampleBuffer) {
        guard let session = session else { return }
        
        let flags = VTDecodeFrameFlags._1xRealTimePlayback
        let status = VTDecompressionSessionDecodeFrame(session, sampleBuffer: sampleBuffer, flags: flags, frameRefcon: nil, infoFlagsOut: nil)

    }
    
    private var addToPreviewStream: ((CGImage) -> Void)?
    
    //  manages the continuous stream of data provided by it
    //  through an AVCaptureVideoDataOutputSampleBufferDelegate object.
    lazy var previewStream: AsyncStream<CGImage> = {
        AsyncStream { continuation in
            addToPreviewStream = { cgImage in
                continuation.yield(cgImage)
            }
        }
    }()
}

var callback: VTDecompressionOutputCallback = { refcon, sourceFrameRefCon, status, infoFlags, imageBuffer, time, duration in
    guard let refcon = refcon,
          status == noErr,
          let imageBuffer = imageBuffer
    else {
        let errorMessage = SecCopyErrorMessageString(status, nil) as String? ?? "Unknown"
        
        print("VTDecompressionOutputCallback \(errorMessage)")
        return
    }
    
    let decoder = Unmanaged<DecompressionManager>.fromOpaque(refcon).takeUnretainedValue()
    decoder.processImage(imageBuffer, time: time, duration: duration)
}
