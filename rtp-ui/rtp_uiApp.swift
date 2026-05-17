//
//  rtp_uiApp.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/7/26.
//

import RTPmacos
import SwiftUI

@main
struct rtp_uiApp: App {
    @State var showingMainMenu: Bool = true
    @State var endpoint: String?

    @State var uploadManagerPtr: OpaquePointer? = nil

    var body: some Scene {
        WindowGroup {
            if showingMainMenu {
                NavigationSplitView(sidebar: {
                    List {
                        NavigationLink("Join", destination: JoinView(state: $showingMainMenu, endpoint: $endpoint))
                        if uploadManagerPtr != nil {
                            NavigationLink("Recordings", destination: FileBrowser(uploadManagerPtr: uploadManagerPtr))
                            NavigationLink("Remote Videos", destination: RemoteVideoView(uploadManagerPtr: uploadManagerPtr))
                            NavigationLink("View Video", destination: VideoSelectionView())
                        }
                    }
                }, detail: {
                    ContentUnavailableView("Easy breezy", systemImage: "figure.dance")
                })
                .frame(minWidth: 500, minHeight: 300)
                .toolbar(removing: .title)
                .task {
                    uploadManagerPtr = rust_setup_docs()
                }
                .onDisappear {
                    rust_deallocate_uploadmanager(uploadManagerPtr)
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
