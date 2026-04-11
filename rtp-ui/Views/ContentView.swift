//
//  ContentView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/7/26.
//

import SwiftUI
import RTPmacos


struct PersonalView: View {
    var viewModel: ViewModel
    
    var body: some View {
        CameraView(image: viewModel.currentFrame)
    }
}

struct PeerVideoView: View {
    var manager: PeerVideoManager
    
    var body: some View {
        ForEach(manager.allPeers) { peer in
            peer
        }
    }
}

struct VideoView: View {
    var peerVideoManager: PeerVideoManager
    var viewModel: ViewModel
    
    var body: some View {
        VideoGrid {
            PersonalView(viewModel: viewModel)
            
            PeerVideoView(manager: peerVideoManager)
        }
        .background(Color.black)
    }
}


struct ContentView: View {
    @State private var viewModel = ViewModel()
    @State private var peerVideoManager = PeerVideoManager()
    
    // MARK: Screen Recording menu
    @State var screenRecorder = ScreenRecorder()
    @State var userStopped = false
    @State var disableInput = false
    @State var recordingMenuOpen = false
    
    var body: some View {
        HStack(spacing: 0){
            VStack(spacing: 0){
                VideoView(peerVideoManager: peerVideoManager, viewModel: viewModel)
                
                UIView()
            }
            
            if recordingMenuOpen {
                ConfigurationView(screenRecorder: screenRecorder, userStopped: $userStopped)
                    .frame(minWidth: 280, maxWidth: 280)
                    .disabled(disableInput)
            }
        }
        .task {
            if await !screenRecorder.canRecord {
                disableInput = true
            }
        }
        .onAppear() {
            peerVideoManager.registerToRust()
        }
        .toolbar(content: {
            Button(action: {
                recordingMenuOpen = !recordingMenuOpen
            }) {
                Label("record", systemImage: "record.circle")
                    .padding(5)
                    .cornerRadius(10)
            }
            .buttonStyle(PlainButtonStyle())
        })
    }
}

#Preview {
    ContentView()
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
