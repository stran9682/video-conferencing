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
        VStack(spacing: 0){
            VideoGrid {
                CameraView(image: $viewModel.currentFrame)
                
                ForEach(peerVideoManager.allPeers) { peer in
                    peer
                }
            }
            .background(Color.black)
            
            UIView()
        }
    }
}

struct VideoGrid: Layout {
    // calculate and report how large a layout container is
    func sizeThatFits(
        proposal: ProposedViewSize,
        subviews: Subviews,
        cache: inout ()
    ) -> CGSize {
        return proposal.replacingUnspecifiedDimensions()
    }
    
    func placeSubviews(
        in bounds: CGRect,
        proposal: ProposedViewSize,
        subviews: Subviews,
        cache: inout ()
    ) {
        let count = subviews.count
        guard count > 0 else { return }

        let columns = Int(ceil(sqrt(Double(count))))
        let rows = Int(ceil(Double(count) / Double(columns)))

        let width = bounds.width / CGFloat(columns)
        let height = bounds.height / CGFloat(rows)

        for (index, subview) in subviews.enumerated() {
            let column = index % columns
            let row = index / columns

            let x = bounds.minX + (CGFloat(column) * width)
            let y = bounds.minY + (CGFloat(row) * height)

            // Place the subview
            subview.place(
                at: CGPoint(x: x, y: y),
                anchor: .topLeading,
                proposal: ProposedViewSize(width: width, height: height)
            )
        }
    }
}
