//
//  rtp_uiApp.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/7/26.
//

import SwiftUI

@main
struct rtp_uiApp: App {
    
    @State var showingMainMenu: Bool = true
    
    var body: some Scene {
        WindowGroup {
            if showingMainMenu {
                NavigationSplitView(sidebar: {
                    List {
                        NavigationLink("Join", destination: JoinView(state: $showingMainMenu))
                        NavigationLink("Recordings", destination: FileBrowser())
                        NavigationLink("Remotes", destination: VideoSelectionView())
                    }
                }, detail: {
                    ContentUnavailableView("Easy breezy", systemImage: "figure.dance")
                })
            }
            else {
                ContentView()
                    .frame(minWidth: 650, minHeight: 500)
            }
        }
        .windowResizability(.contentSize)
        .windowToolbarStyle(.unifiedCompact)
    }
}
