//
//  rtp_uiApp.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/7/26.
//

import SwiftUI

@main
struct rtp_uiApp: App {
    
    @State var showingJoinScreen: Bool = true
    
    var body: some Scene {
        WindowGroup {
            if showingJoinScreen {
                JoinView(state: $showingJoinScreen)
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
