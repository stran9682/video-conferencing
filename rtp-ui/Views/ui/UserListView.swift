//
//  UserListView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 5/13/26.
//

import SwiftUI
import RTPmacos

struct SharedWithListItemView: View {
    
    init(users: [String], namespace_id: String, uploadManagerPtr: OpaquePointer?) {
        mockData = users.map({ user in
            UserRowModel(user: user)
        })
        
        self.namespace_id = namespace_id
        self.uploadManagerPtr = uploadManagerPtr
    }

    private var namespace_id: String
    private var uploadManagerPtr: OpaquePointer? = nil
    
    @State private var newViewer = ""
    @State private var mockData: [UserRowModel]
    
    var body: some View {
        VStack {
            HStack(alignment: .center) {
                
                // TODO: replace with the name of the video
                Text("\(namespace_id)")
                    .font(Font.title.bold())

                Spacer()

                Button(action: {
                    mockData.removeAll(where: { $0.toBeRemoved == true })
                
                    let authorizedUsers = AuthorizedUsers(
                        namespace_id: namespace_id, authorized_users: mockData.map({ $0.user })
                    )
                    
                    let encoder = JSONEncoder()
                    do {
                        guard uploadManagerPtr != nil else { return }
                        
                        let jsonData = try encoder.encode(authorizedUsers)
                        
                        let byteArray = [UInt8](jsonData)
                        
                        rust_change_permissions(uploadManagerPtr, byteArray, UInt(byteArray.count))
                        
                    } catch {
                        print("Serialization failed: \(error)")
                    }

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
