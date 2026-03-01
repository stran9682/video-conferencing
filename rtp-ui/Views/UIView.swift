//
//  UIView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 2/28/26.
//

import SwiftUI

struct UIView: View {
    var body: some View {
        HStack {
            Button(action: {
                print("HI")
            }) {
                Label("End Call", systemImage: "phone.down")
                    .padding(10)
                    .background(.red)
                    .cornerRadius(10)
            }
            .buttonStyle(PlainButtonStyle())
            
            Spacer()
            
            Button(action: {
                print("HI")
            }) {
                Label("Mute", systemImage: "microphone.slash")
                    .padding(10)
                    .background(.gray.opacity(0.3))
                    .cornerRadius(10)
            }
            .buttonStyle(PlainButtonStyle())
            
            Button(action: {
                print("HI")
            }) {
                Label("Disable Video", systemImage: "video.slash")
                    .padding(10)
                    .background(.gray.opacity(0.3))
                    .cornerRadius(10)
            }
            .buttonStyle(PlainButtonStyle())
            
            Spacer()
            
            
            Button(action: {
                print("HI")
            }) {
                Label("Copy Invite", systemImage: "person.crop.circle.badge.plus")
                    .padding(10)
                    .background(.green)
                    .cornerRadius(10)
            }
            .buttonStyle(PlainButtonStyle())
        }
        .padding()
        .background(.gray.opacity(0.01))

    }
}

#Preview {

    UIView()
}
