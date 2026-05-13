//
//  ContentView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/7/26.
//

import RTPmacos
import SwiftUI

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
    var endpoint: String?

    @State private var viewModel = ViewModel()
    @State private var peerVideoManager = PeerVideoManager()

    // MARK: Screen Recording menu

    @State var screenRecorder = ScreenRecorder()
    @State var disableInput = false
    @State var recordingMenuOpen = false

    @State var showOverlay: Bool? = nil

    var body: some View {
        HStack(spacing: 0) {
            VStack(spacing: 0) {
                VideoView(peerVideoManager: peerVideoManager, viewModel: viewModel)

                UIView(action: { result in
                    Task {
                        withAnimation(.easeInOut(duration: 0.2)) {
                            showOverlay = result
                        }

                        try? await Task.sleep(for: .seconds(0.5))

                        withAnimation(.easeInOut(duration: 0.2)) {
                            showOverlay = nil
                        }
                    }
                })
            }
            .overlay(alignment: .top, content: {
                if showOverlay != nil, showOverlay! {
                    Text("Copied to clipboard!")
                        .padding()
                        .background(Color.green.opacity(0.7))
                        .foregroundColor(.white)
                        .cornerRadius(10)
                        .transition(.move(edge: .top).combined(with: .opacity))
                } else if showOverlay != nil {
                    Text("Not quite ready yet")
                        .padding()
                        .background(Color.red.opacity(0.7))
                        .foregroundColor(.white)
                        .cornerRadius(10)
                        .transition(.move(edge: .top).combined(with: .opacity))
                }
            })

            if recordingMenuOpen {
                ConfigurationView(screenRecorder: screenRecorder)
                    .frame(minWidth: 280, maxWidth: 280)
                    .disabled(disableInput)
                    .transition(.move(edge: .trailing).combined(with: .opacity))
            }
        }
        .task {
            if await !screenRecorder.canRecord {
                disableInput = true
            } else {
                await screenRecorder.monitorAvailableContent()
            }
        }
        .onAppear {
            rust_run_network_runtime(endpoint, UInt(endpoint?.count ?? 0))
            peerVideoManager.registerToRust()
        }
        .toolbar(content: {
            Spacer()
            Button(action: {
                withAnimation(.easeInOut(duration: 0.2)) {
                    recordingMenuOpen = !recordingMenuOpen
                }

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
    /// calculate and report how large a layout container is
    func sizeThatFits(
        proposal: ProposedViewSize,
        subviews _: Subviews,
        cache _: inout ()
    ) -> CGSize {
        return proposal.replacingUnspecifiedDimensions()
    }

    func placeSubviews(
        in bounds: CGRect,
        proposal _: ProposedViewSize,
        subviews: Subviews,
        cache _: inout ()
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
