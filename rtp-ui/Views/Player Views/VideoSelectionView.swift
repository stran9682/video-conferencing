//
//  VideoSelectionView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 4/22/26.
//

import SwiftUI

struct VideoSelectionView: View {
    
    @State private var tag = ""
    @State private var endpoint = ""
    @State private var isPlaying = false
    
    var body: some View {
        if !isPlaying {
            VStack {
                TextField("Enter endpoint", text: $endpoint)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 200)
                
                TextField("Enter tag", text: $tag)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 200)
                
                Button(action: {
                    isPlaying = true
                }, label: {
                    Text("Submit")
                })
            }
        } else {
            PlayerView(tag: tag, endpoint: endpoint)
        }
    }
}

#Preview {
    VideoSelectionView()
}
