//
//  UserListView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 5/13/26.
//

import SwiftUI

struct UserListView: View {
    // TODO: replace this with nothing
    @State var mockData: [UserRowModel] = []

    @State private var newViewer = ""

    var body: some View {
        VStack {
            HStack(alignment: .center) {
                Text("Users")
                    .font(Font.title.bold())

                Spacer()

                Button(action: {
                    mockData.removeAll(where: { $0.toBeRemoved == true })

                }, label: {
                    Label("Save", systemImage: "square.and.arrow.down")
                        .foregroundStyle(Color(.white))
                })
                .buttonStyle(.borderedProminent)
                .background(Color(.systemBlue), in: .buttonBorder)
            }

            List(mockData) { user in
                UserRowView(user: user)
            }

            HStack {
                TextField("Enter ID", text: $newViewer)
                    .textFieldStyle(RoundedBorderTextFieldStyle())

                Button(action: {
                    guard !newViewer.trimmingCharacters(in: .whitespaces).isEmpty else { return }

                    if mockData.contains(where: { $0.user == newViewer }) { return }

                    mockData.append(UserRowModel(user: newViewer))

                    newViewer = ""
                }, label: {
                    Label("Add", systemImage: "person.fill.badge.plus")
                        .foregroundStyle(Color(.white))
                })
                .buttonStyle(.borderedProminent)
                .disabled(newViewer.isEmpty)
            }
            .padding(10)
        }
        .task {
            // TODO: load stuff from rust.
        }
    }
}

struct UserRowView: View {
    @State var user: UserRowModel

    var body: some View {
        HStack {
            Text(user.user)

            Spacer()

            Toggle("remove", isOn: $user.toBeRemoved)
                .toggleStyle(.checkbox)
        }
    }
}

@Observable
class UserRowModel: Identifiable {
    let id = UUID()
    var user: String
    var toBeRemoved: Bool = false

    init(user: String) {
        self.user = user
    }
}

#Preview {
    UserListView()
}
