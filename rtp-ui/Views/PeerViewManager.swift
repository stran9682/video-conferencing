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
    
    // observing this in particular to get video feeds of our peers
    private var peers: Dictionary<String, PeerView> = [:]
    
    init() {
        
    }
    
    func addPeer(peerView : PeerView, address: String) {
        
        DispatchQueue.main.async {
            self.peers[address] = peerView
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
) -> UnsafeMutableRawPointer? {
    guard
        let context = context,
        let addr = addr
    else { return nil }
    
    let peerVideoManager = Unmanaged<PeerVideoManager>.fromOpaque(context).takeUnretainedValue()
    
    // copy the data - rust will drop the original
    let pps = Array(UnsafeBufferPointer(start: pps, count: Int(ppsLength)))
    let sps = Array(UnsafeBufferPointer(start: sps, count: Int(spsLength)))
    let address = String(cString: addr)
    
    let model = PeerVideoModel(pps: pps, sps: sps)
    let view = PeerView(peerVideoModel: model)
    
    peerVideoManager.addPeer(peerView: view, address: address)

    // MARK: return the pointer of the peer model
    return Unmanaged.passRetained(model).toOpaque()
}

