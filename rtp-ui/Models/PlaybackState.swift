//
//  PlaybackState.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 4/20/26.
//

import AVFoundation
import Foundation
import RTPmacos

@Observable
class PlaybackState {
    // TODO: Try using an AVMutableComposition instead
    var player: AVQueuePlayer = .init()

    private var endpoint: String
    private var tag: String
    private var clipNumber: Int32 = 0
    private var clipCount: Int32

    init(tag: String, clipCount: Int32, endpoint: String) {
        self.endpoint = endpoint

        self.tag = tag // the uuid common to all the files
        self.clipCount = clipCount // the total number of clips

        player.automaticallyWaitsToMinimizeStalling = false

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleClipFinished),
            name: AVPlayerItem.didPlayToEndTimeNotification,
            object: nil
        )

        bufferClips()
    }

    @objc func handleClipFinished(notification _: Notification) {
        bufferClips()
    }

    func bufferClips() {
        if clipNumber > clipCount || clipCount == 0 {
            return
        }

        let query = "\(tag):\(clipNumber)"

        print("Querying for \(query)")

        swift_download(
            query,
            UInt(query.count),
            endpoint,
            UInt(endpoint.count),
            Unmanaged.passRetained(self).toOpaque()
        )

        clipNumber += 1
    }
}

@_cdecl("swift_receive_video")
public func swift_receive_video(
    _ context: UnsafeMutableRawPointer?,
    _ path: UnsafePointer<UInt8>?
) {
    guard let context = context, let path else { return }

    let uuid = String(cString: path)

    let playbackState = Unmanaged<PlaybackState>.fromOpaque(context).takeRetainedValue()

    let directory = URL.temporaryDirectory.appendingPathComponent("\(uuid)")

    let playerItem = AVPlayerItem(url: directory)

    DispatchQueue.main.async {
        playbackState.player.insert(playerItem, after: nil)
    }
}

@_cdecl("swift_release_pointer")
public func swift_receive_video(
    _ context: UnsafeMutableRawPointer?
) {
    guard let context else { return }

    _ = Unmanaged<PlaybackState>.fromOpaque(context).takeRetainedValue()
}
