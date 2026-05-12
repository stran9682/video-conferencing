//
//  UIView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 2/28/26.
//

import Darwin
import RTPmacos
import SwiftUI

struct UIView: View {
    let action: (Bool) -> Void

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
                action(copyInvite())
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

    func copyInvite() -> Bool {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()

        var buffer = [Int8](repeating: 0, count: 256)

        if rust_get_address(&buffer) {
            let address = String(cString: buffer)

            print(address)

            pasteboard.setString(address, forType: .string)

            return true
        }

        return false
    }
}
