//
//  PeerVideoModel.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/31/26.
//

import Foundation
import CoreImage
import VideoToolbox

class PeerVideoModel: Hashable {
    
    var currentFrame: CGImage?
    
    private var decompressionManager: DecompressionManager
    private var sps : [UInt8] = []
    private var pps : [UInt8] = []

    init(pps: [UInt8], sps: [UInt8]) {
        self.pps = pps
        self.sps = sps
        
        decompressionManager = DecompressionManager(
            sps: self.sps,
            spsLength: sps.count,
            pps: self.pps,
            ppsLength: pps.count
        )
        
        Task {
            await handleFramePreviews()
        }
    }
    
    // update image every time frame is done being processed
    func handleFramePreviews() async {
        for await image in decompressionManager.previewStream {
            Task {
                @MainActor in currentFrame = image
            }
        }
    }
    
    // every time a frame comes in, place into decompression manager
    func decompressFrame(_ frameArguments: FrameArguments?) {
        decompressionManager.decode(sampleBuffer: <#T##CMSampleBuffer#>)
    }
 
    // hasheable stuff
    static func == (lhs: PeerVideoModel, rhs: PeerVideoModel) -> Bool {
        ObjectIdentifier(lhs) == ObjectIdentifier(rhs)
    }

    func hash(into hasher: inout Hasher) {
        hasher.combine(ObjectIdentifier(self))
    }
}

@_cdecl("swift_receive_frame")
public func swift_receive_frame(
    _ context: UnsafeMutableRawPointer?,
    _ frameData: UnsafePointer<UInt8>?
) {
    guard let context = context, let frameData = frameData else { return }
    
    let peerVideoModel = Unmanaged<PeerVideoModel>.fromOpaque(context).takeUnretainedValue()
    
    // MARK: Send to be decompressed.
    peerVideoModel.decompressFrame(frameData)
}
