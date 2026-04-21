//
//  PlaybackState.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 4/20/26.
//

import Foundation
import AVFoundation
import RTPmacos

@Observable
class PlaybackState {
    var player: AVQueuePlayer
    var endpoint: String
    
    // just be careful, these are modified off the main thread,
    // don't observe them.
    var clipNumber: Int32 = 0
    var hashes: [String] = []
    
    init(hashSequence: String, endpoint: String) {
        self.endpoint = endpoint
        player = AVQueuePlayer()
        player.automaticallyWaitsToMinimizeStalling = false
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleClipFinished),
            name: .AVPlayerItemDidPlayToEndTime,
            object: nil
        )
        
        // TODO: Start playback by passing a hash to rust
        // TODO: Handle receiving hashes and playback state
        
        swift_download(
            hashSequence,
            UInt(hashSequence.count),
            endpoint, UInt(endpoint.count),
            Unmanaged.passRetained(self).toOpaque(),
            true
        )
    }

    @objc func handleClipFinished(notification: Notification) {
        if clipNumber < hashes.count {
            swift_download(
                self.hashes[Int(self.clipNumber)],
                UInt(self.hashes[Int(self.clipNumber)].count),
                self.endpoint,
                UInt(self.endpoint.count),
                Unmanaged.passRetained(self).toOpaque(),
                false
            )
            clipNumber += 1
        }
    }
}

@_cdecl("swift_receive_hashes")
public func swift_receive_hashes(
    _ context: UnsafeMutableRawPointer?,
    _ hashes: UnsafePointer<UInt8>?,
    _ count: UInt
) {
    guard let context = context, let hashes = hashes else { return }
    
    let playbackState = Unmanaged<PlaybackState>.fromOpaque(context).takeRetainedValue()
    
    for hash in stride(from: 0, to: count, by: 64) {
        let currentChunkPtr = hashes.advanced(by: Int(hash))
        let actualChunkSize = min(64, count - hash)
        let buffer = UnsafeBufferPointer(start: currentChunkPtr, count: Int(actualChunkSize))
        playbackState.hashes.append(String(decoding: buffer, as: UTF8.self))
        
        let strHash = String(decoding: buffer, as: UTF8.self)
    }
    
    print(playbackState.hashes)

    swift_download(
        playbackState.hashes[0],
        UInt(playbackState.hashes[0].count),
        playbackState.endpoint,
        UInt(playbackState.endpoint.count),
        Unmanaged.passRetained(playbackState).toOpaque(),    // 😤
        false
    )
}

@_cdecl("swift_receive_video")
public func swift_receive_video(
    _ context: UnsafeMutableRawPointer?,
    _ path: UnsafePointer<UInt8>?,
) {
    guard let context = context, let path else { return }
    
    let uuid = String(cString: path)
    
    let playbackState = Unmanaged<PlaybackState>.fromOpaque(context).takeRetainedValue()
    
    let directory =  URL.temporaryDirectory.appendingPathComponent("\(uuid)")

    let playerItem = AVPlayerItem(url: directory)
    
    DispatchQueue.main.async {
        playbackState.player.insert(playerItem, after: nil)
        playbackState.clipNumber += 1
    }
}
