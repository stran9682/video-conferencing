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
    
    // just be careful, these are modified off the main thread,
    // don't observe them.
    var clipNumber: Int32 = 0
    var hashes: [String] = []
    
    init(hashSequence: String, endpoint: String) {
        player = AVQueuePlayer()
        player.play()
        
        
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
            Unmanaged.passUnretained(self).toOpaque()
        )
    }

    @objc func handleClipFinished(notification: Notification) {
        // tell rust to get the next clip
    }
}

@_cdecl("swift_receive_hashes")
public func swift_receive_hashes(
    _ context: UnsafeMutableRawPointer?,
    _ hashes: UnsafePointer<UInt8>?,
    _ count: UInt
) {
    guard let context = context, let hashes = hashes else { return }
    
    let playbackState = Unmanaged<PlaybackState>.fromOpaque(context).takeUnretainedValue()
    
    for hash in stride(from: 0, to: count, by: 32) {
        let currentChunkPtr = hashes.advanced(by: Int(hash))
        let actualChunkSize = min(32, count - hash)
        let buffer = UnsafeBufferPointer(start: currentChunkPtr, count: Int(actualChunkSize))
        playbackState.hashes.append(String(decoding: buffer, as: UTF8.self))
    }
    
    print(playbackState.hashes)

    // TODO: request the first video.
}

@_cdecl("swift_receive_video")
public func swift_receive_video(
    _ context: UnsafeMutableRawPointer?,
) {
    guard let context = context else { return }
    
    let playbackState = Unmanaged<PlaybackState>.fromOpaque(context).takeUnretainedValue()
    
    let directory = URL.documentsDirectory.appending(component: "/temp/\(playbackState.clipNumber).mp4")
    let playerItem = AVPlayerItem(url: URL(filePath: directory.relativePath))
    
    DispatchQueue.main.async {
        playbackState.player.insert(playerItem, after: nil)
        playbackState.clipNumber += 1
    }
}
