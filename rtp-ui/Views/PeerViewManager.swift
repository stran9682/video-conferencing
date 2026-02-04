//
//  PeerVideoManager.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/31/26.
//

import Foundation
import RTPmacos


@Observable
class PeerVideoManager {
    
    private var peers: Dictionary<String, PeerView> = [:]
    
    init() {
        // send the context to rust!
        let refcon = Unmanaged.passUnretained(self).toOpaque()
        rust_send_video_callback(refcon)
    }
    
    func addPeer(pps: [UInt8], sps: [UInt8], address: String) {
        let newView = PeerView(peerVideoModel: PeerVideoModel(pps: pps, sps: sps))
        
        DispatchQueue.main.async {
            self.peers[address] = newView
        }
        
    }
}

@_cdecl("swift_receive_pps_sps")
public func swift_receive_pps_sps(
    _ context: UnsafeMutableRawPointer?,
    _ pps: UnsafePointer<UInt8>?,
    _ ppsLength: UInt,
    _ sps: UnsafePointer<UInt8>?,
    _ spsLength: UInt,
    _ addr: UnsafePointer<UInt8>?,
) {
    guard
        let context = context,
        let addr = addr
    else { return }
    
    let peerVideoManager = Unmanaged<PeerVideoManager>.fromOpaque(context).takeUnretainedValue()
    
    // copy the data - rust will drop the original
    let pps = Array(UnsafeBufferPointer(start: pps, count: Int(ppsLength)))
    let sps = Array(UnsafeBufferPointer(start: sps, count: Int(spsLength)))
    let address = String(cString: addr)
    
    peerVideoManager.addPeer(pps: pps, sps: sps, address: address)
   
    // MARK: return the pointer of the peer
}

