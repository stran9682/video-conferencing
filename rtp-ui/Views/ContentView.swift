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
    
    let layout = [
       GridItem(.flexible(), spacing: 5),
       GridItem(.flexible(), spacing: 5)
   ]
    
    var body: some View {
        LazyVGrid(columns: layout) {
            CameraView(image: $viewModel.currentFrame)
                .frame(width:300, height: 300)
            
            ForEach(peerVideoManager.allPeers) { peer in
                peer
            }
        }
//        
//        
//        List(peerVideoManager.allPeers) { peer in
//            peer
//        }
    }
}
