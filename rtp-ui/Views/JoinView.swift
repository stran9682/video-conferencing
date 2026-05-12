//
//  JoinView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/10/26.
//

import RTPmacos
import SwiftUI

struct JoinView: View {
    @Binding var state: Bool
    @Binding var endpoint: String?
    @State private var address = ""

    var body: some View {
        VStack {
            Button(action: {
                state = false
            }, label: {
                Text("Start Session")
            })
            .padding()

            TextField("Enter SIP address", text: $address)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 200)

            Button(action: {
                endpoint = address
                state = false
            }, label: {
                Text("Submit")
            })
        }
        .frame(minWidth: 500, minHeight: 300)
    }
}
