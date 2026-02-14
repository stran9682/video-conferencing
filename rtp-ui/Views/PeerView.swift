//
//  PeerView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 2/4/26.
//

import SwiftUI

struct PeerView: View {
    @State private var peerVideoModel : PeerVideoModel
    
    init(peerVideoModel: PeerVideoModel) {
        self.peerVideoModel = peerVideoModel
    }
    
    var body: some View {
        CameraView(image: $peerVideoModel.currentFrame)
            .frame(width: 300, height: 300)
    }
}
