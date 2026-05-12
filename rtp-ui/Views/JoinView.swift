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
        HStack {
            VStack {
                Text("Begin a call")
                    .font(.largeTitle)
                    .frame(maxWidth: .infinity)

                Button(action: {
                    state = false
                }, label: {
                    Text("Start Session")
                        .frame(maxWidth: .infinity)
                        .frame(height: 30)
                        .foregroundStyle(Color(.white))
                })
                .buttonStyle(.borderless)
                .background(Color(.green), in: .buttonBorder)
                .frame(maxWidth: .infinity)
            }
            .padding(20)

            Divider()

            VStack {
                Text("Or join a session")
                    .font(.largeTitle)
                    .frame(maxWidth: .infinity)

                TextField("Enter Endpoint", text: $address)
                    .textFieldStyle(.roundedBorder)

                Button(action: {
                    endpoint = address
                    state = false
                }, label: {
                    Text("Submit")
                        .frame(maxWidth: .infinity)
                        .frame(height: 30)
                        .foregroundStyle(Color(.white))
                })
                .buttonStyle(.borderless)
                .background(Color(.systemBlue), in: .buttonBorder)
            }
            .padding(20)
        }
    }
}

#Preview {
    @Previewable @State var state = false
    @Previewable @State var endpoint: String? = nil
    JoinView(state: $state, endpoint: $endpoint)
}
