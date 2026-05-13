//
//  PlayerView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 4/20/26.
//

import AVFoundation
import AVKit
import SwiftUI

struct PlayerView: View {
    var tag: String
    var endpoint: String

    @State private var player: PlaybackState?
    @State private var isPlaying = false

    var body: some View {
        VStack {
            if let player {
                VideoPlayer(player: player.player)

                Button {
                    isPlaying ? player.player.pause() : player.player.play()
                    isPlaying.toggle()
                } label: {
                    Image(systemName: isPlaying ? "stop" : "play")
                        .padding()
                }
            }
        }
        .task {
            let query = tag.split(separator: ":")

            if query.count == 2 {
                let tagQuery = String(query[0])
                let clipCountQuery = Int32(query[1]) ?? 0

                player = PlaybackState(tag: tagQuery, clipCount: clipCountQuery, endpoint: endpoint)
            }
        }
        .onDisappear {
            let fileManager = FileManager.default
            let path = URL.temporaryDirectory.appendingPathComponent("clips")
            try? fileManager.removeItem(at: path)
        }
    }
}
