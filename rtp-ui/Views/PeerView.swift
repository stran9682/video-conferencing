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
        Text(/*@START_MENU_TOKEN@*/"Hello, World!"/*@END_MENU_TOKEN@*/)
    }
}
