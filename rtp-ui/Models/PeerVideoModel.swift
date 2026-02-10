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
        
        var timingInfo = CMSampleTimingInfo(
            duration: .invalid,
            presentationTimeStamp: .invalid, // Or your actual RTP timestamp
            decodeTimeStamp: .invalid
        )

        let status = CMSampleBufferCreateReady(
            allocator: kCFAllocatorDefault,
            dataBuffer: blockBuffer,
            formatDescription: decompressionManager.formatDescription,
            sampleCount: 1,
            sampleTimingEntryCount: 1,
            sampleTimingArray: &timingInfo,
            sampleSizeEntryCount: 0,
            sampleSizeArray: nil,
            sampleBufferOut: &sampleBuffer
        )
        
        if let sampleBuffer = sampleBuffer, status == noErr{
            decompressionManager.decode(sampleBuffer: sampleBuffer)
        }
        else {
            print("\(status)")
        }
    }
}


func auditNALU(data: UnsafeMutableRawPointer, length: Int) {
    // 1. Move past the 4-byte AVCC length header
    let payload = data.assumingMemoryBound(to: UInt8.self).advanced(by: 4)
    
    // 2. The first byte of the NALU is the header
    let headerByte = payload.pointee
    
    // 3. Extract the NALU Type (lower 5 bits)
    let naluType = headerByte & 0x1F
    
    let typeName: String
    switch naluType {
    case 1:  typeName = "P-Frame (Non-IDR Slice)"
    case 5:  typeName = "IDR-Frame (Key Frame)"
    case 6:  typeName = "SEI (Supplemental Information)"
    case 7:  typeName = "SPS (Sequence Parameter Set)"
    case 8:  typeName = "PPS (Picture Parameter Set)"
    case 9:  typeName = "AUD (Access Unit Delimiter)"
    default: typeName = "Unknown (\(naluType))"
    }
    
    print("NALU Type: \(naluType) [\(typeName)] | Header Byte: \(String(format: "%02X", headerByte))")
}

@_cdecl("swift_receive_frame")
public func swift_receive_frame(
    _ context: UnsafeMutableRawPointer?,
    _ frameData: UnsafeMutableRawPointer?,
    _ frameDataLength: UInt
) {
    guard let context = context, let frameData = frameData else { return }
    
    auditNALU(data: frameData, length: Int(frameDataLength))
    
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
