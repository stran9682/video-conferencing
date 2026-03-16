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
    private var peers: Dictionary<UInt32, PeerView> = [:]
    
    var allPeers: [PeerView] {
        Array(peers.values)
    }
    
    func addPeer(peerView : PeerView, ssrc: UInt32) {
        
        DispatchQueue.main.async {
            self.peers[ssrc] = peerView
        }
        
    }
    
    func removePeer(ssrc: UInt32) {
        DispatchQueue.main.async {
            self.peers.removeValue(forKey: ssrc)
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
    _ ssrc: UInt32
) -> UnsafeMutableRawPointer? {
    guard let context = context else { return nil }
    
    let peerVideoManager = Unmanaged<PeerVideoManager>.fromOpaque(context).takeUnretainedValue()
    
    // copy the data - rust will drop the original
    let pps = Array(UnsafeBufferPointer(start: pps, count: Int(ppsLength)))
    let sps = Array(UnsafeBufferPointer(start: sps, count: Int(spsLength)))
    
    let model = PeerVideoModel(pps: pps, sps: sps)
    let view = PeerView(peerVideoModel: model)
    
    peerVideoManager.addPeer(peerView: view, ssrc: ssrc)

    // MARK: return the pointer of the peer model
    return Unmanaged.passRetained(model).toOpaque()
}

@_cdecl("swift_remove_video_peer")
public func swift_remove_video_peer(
    _ ssrc: UInt32,
    _ video_manager_context: UnsafeMutableRawPointer?,
    _ peer_context: UnsafeMutableRawPointer?
) {
    guard let video_manager_context, let peer_context else { return }
    
    let peerVideoManager = Unmanaged<PeerVideoManager>.fromOpaque(video_manager_context).takeUnretainedValue()
    
    peerVideoManager.removePeer(ssrc: ssrc)
    
    let _ = Unmanaged<ParticipantAudio>.fromOpaque(peer_context).takeRetainedValue()
}
