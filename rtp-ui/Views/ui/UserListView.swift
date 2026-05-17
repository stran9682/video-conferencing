//
//  UserListView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 5/13/26.
//

import RTPmacos
import SwiftUI

struct SharedWithListItemView: View {
    init(
        users: [String],
        namespace_id: String,
        uploadManagerPtr: OpaquePointer?
    ) {
        mockData = users.map { user in
            UserRowModel(user: user)
        }

        self.namespace_id = namespace_id
        self.uploadManagerPtr = uploadManagerPtr
    }

    private var namespace_id: String
    private var uploadManagerPtr: OpaquePointer?

    @State private var newViewer = ""
    @State private var mockData: [UserRowModel]

    var body: some View {
        VStack {
            HStack {
                // TODO: replace with the name of the video
                Text("\(namespace_id)")
                    .font(Font.title2.bold())

                Spacer()
            }
            .padding(10)

            List(mockData) { user in
                UserRowView(user: user)
            }

            HStack {
                TextField("Enter ID", text: $newViewer)
                    .controlSize(.large)
                    .textFieldStyle(RoundedBorderTextFieldStyle())

                Button(
                    action: {
                        guard
                            !newViewer.trimmingCharacters(in: .whitespaces)
                            .isEmpty
                        else { return }

                        if mockData.contains(where: { $0.user == newViewer }) {
                            return
                        }

                        mockData.append(UserRowModel(user: newViewer))

                        newViewer = ""
                    },
                    label: {
                        Label(
                            "Add",
                            systemImage: "person.crop.circle.badge.plus"
                        )
                        .foregroundStyle(Color(.white))
                        .padding(5)
                    }
                )
                .buttonStyle(.borderless)
                .background(Color(.mint), in: .buttonBorder)
                .disabled(newViewer.isEmpty)

                Button(
                    action: {
                        mockData.removeAll(where: { $0.toBeRemoved == true })

                        let authorizedUsers = AuthorizedUsers(
                            namespace_id: namespace_id,
                            authorized_users: mockData.map { $0.user }
                        )

                        let encoder = JSONEncoder()
                        do {
                            guard uploadManagerPtr != nil else { return }

                            let jsonData = try encoder.encode(authorizedUsers)

                            let byteArray = [UInt8](jsonData)

                            rust_change_permissions(
                                uploadManagerPtr,
                                byteArray,
                                UInt(byteArray.count)
                            )

                        } catch {
                            print("Serialization failed: \(error)")
                        }

                    },
                    label: {
                        Label("Save", systemImage: "square.and.arrow.down")
                            .foregroundStyle(Color(.white))
                            .padding(5)
                    }
                )
                .buttonStyle(.borderless)
                .background(Color(.mint), in: .buttonBorder)

                Button {
                    let _ = copyInvite(namespace_id: namespace_id)
                } label: {
                    Label("Copy Share Code", systemImage: "plus.app")
                        .foregroundStyle(Color(.white))
                        .padding(5)
                }
                .buttonStyle(.borderless)
                .background(Color(.orange), in: .buttonBorder)
            }
            .padding(10)
        }
    }

    func copyInvite(namespace_id: String) -> Bool {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()

        var buffer = [Int8](repeating: 0, count: 256)

        if rust_get_doc_ticket(
            uploadManagerPtr,
            namespace_id,
            UInt(namespace_id.count),
            &buffer
        ) {
            let ticket = String(cString: buffer)

            print("Swift side: \(ticket)")

            pasteboard.setString(ticket, forType: .string)

            return true
        }

        return false
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
    SharedWithListItemView(
        users: ["Steve", "Tim", "Bill"],
        namespace_id: "123",
        uploadManagerPtr: nil
    )
}
