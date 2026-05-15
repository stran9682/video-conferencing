//
//  rtp_uiApp.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/7/26.
//

import SwiftUI
import RTPmacos

@main
struct rtp_uiApp: App {
    @State var showingMainMenu: Bool = true
    @State var endpoint: String?

    var body: some Scene {
        WindowGroup {
            if showingMainMenu {
                NavigationSplitView(sidebar: {
                    List {
                        NavigationLink("Join", destination: JoinView(state: $showingMainMenu, endpoint: $endpoint))
                        NavigationLink("Recordings", destination: FileBrowser())
                        NavigationLink("Remote Videos", destination: RemoteVideoView())
                        NavigationLink("View Video", destination: VideoSelectionView())
                    }
                }, detail: {
                    ContentUnavailableView("Easy breezy", systemImage: "figure.dance")
                })
                .frame(minWidth: 500, minHeight: 300)
                .toolbar(removing: .title)
                .onAppear {
                    rust_setup_docs()
                    print("i have appeared")
                }
            } else {
                ContentView(endpoint: endpoint)
                    .frame(minWidth: 650, minHeight: 500)
                    .toolbar(removing: .title)
            }
        }
        .windowResizability(.contentSize)
    }
}
