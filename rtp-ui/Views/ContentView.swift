//
//  ContentView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/7/26.
//

import SwiftUI
import RTPmacos


struct ContentView: View {
    @State private var viewModel = ViewModel()
    
    @State private var peerVideoManager = PeerVideoManager()
    
    init () {
        // send the context to rust!
        let refcon = Unmanaged.passRetained(peerVideoManager).toOpaque()
        rust_send_video_callback(refcon)
    }
    
    var body: some View {
        CameraView(image: $viewModel.currentFrame)
    }
}
