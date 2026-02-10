//
//  PeerVideoModel.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/31/26.
//

import Foundation
import CoreImage
import VideoToolbox

@Observable
class PeerVideoModel {
    
    var currentFrame: CGImage?
    
    private var decompressionManager: DecompressionManager

    init(pps: [UInt8], sps: [UInt8]) {
        
        decompressionManager = DecompressionManager(
            sps: sps,
            spsLength: sps.count,
            pps: pps,
            ppsLength: pps.count
        )
        
        Task {
            await handleFramePreviews()
        }
    }
    
    // update image every time frame is done being processed
    func handleFramePreviews() async {
        for await image in decompressionManager.previewStream {
            Task { @MainActor in
                currentFrame = image
            }
        }
    }
    
    // every time a frame comes in, place into decompression manager
    func decompressFrame(blockBuffer : CMBlockBuffer) {
        var sampleBuffer: CMSampleBuffer?
        
        let status = CMSampleBufferCreate(allocator: kCFAllocatorDefault, dataBuffer: blockBuffer, dataReady: true, makeDataReadyCallback: nil, refcon: nil, formatDescription: decompressionManager.formatDescription, sampleCount: 1, sampleTimingEntryCount: 0, sampleTimingArray: nil, sampleSizeEntryCount: 0, sampleSizeArray: nil, sampleBufferOut: &sampleBuffer)
        
        if let sampleBuffer = sampleBuffer, status == noErr{
            decompressionManager.decode(sampleBuffer: sampleBuffer)
        }
        else {
            print("\(status)")
        }
    }
}

@_cdecl("swift_receive_frame")
public func swift_receive_frame(
    _ context: UnsafeMutableRawPointer?,
    _ frameData: UnsafeMutableRawPointer?,
    _ frameDataLength: UInt
) {
    guard let context = context, let frameData = frameData else { return }
    
    let peerVideoModel = Unmanaged<PeerVideoModel>.fromOpaque(context).takeUnretainedValue()
    
    // TODO: I'm copying for now, but look into a zero copy solution.
    let frameDataCopy = UnsafeMutableRawPointer.allocate(byteCount: Int(frameDataLength), alignment: 16)
    
    frameDataCopy.copyMemory(from: frameData, byteCount: Int(frameDataLength))
    
    var blockBuffer: CMBlockBuffer?
    
    let status = CMBlockBufferCreateWithMemoryBlock(
        allocator: kCFAllocatorDefault,
        memoryBlock: frameDataCopy,
        blockLength: Int(frameDataLength),
        blockAllocator: kCFAllocatorDefault,
        customBlockSource: nil,
        offsetToData: 0,
        dataLength: Int(frameDataLength),
        flags: 0,
        blockBufferOut: &blockBuffer)
    
    if status == noErr, let blockBuffer = blockBuffer {
        // MARK: Send to be decompressed.
        peerVideoModel.decompressFrame(blockBuffer: blockBuffer)
    }
}
