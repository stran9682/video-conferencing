//
//  UIView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 2/28/26.
//

import SwiftUI
import Darwin

struct UIView: View {
    var body: some View {
        HStack {
            Button(action: {
                exit(0)
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
