//
//  PlayerView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 4/20/26.
//

import SwiftUI
import AVFoundation
import AVKit

struct PlayerView: View {
    @State private var player: PlaybackState?
    @State private var isPlaying = false


    var body: some View {
        VStack {
            if let player {
                VideoPlayer(player: AVQueuePlayer())
                    .frame(width: 320, height: 180, alignment: .center)
                
                Button {
                    isPlaying ? player.player.pause() : player.player.play()
                    isPlaying.toggle()
                    player.player.seek(to: .zero)
                } label: {
                    Image(systemName: isPlaying ? "stop" : "play")
                        .padding()
                }
            }
        }
        .task {
            // Use the task modifier to defer creating the player to ensure
            // SwiftUI creates it only once when it first presents the view.
            // TODO: Pass the hashsequence hash to begin
            player = PlaybackState(hashSequence: "cb10a7cae221a1bfcc7653c378ad5c188d6c49d399ca69b042ff854d5d386625", endpoint: "3de13357f5eba5f7bc15eeb164669f34cfb92fec5c48ebac8c4b04234914b662")
        }
    }
}
