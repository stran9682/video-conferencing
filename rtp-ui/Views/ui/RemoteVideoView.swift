//
//  RemoteVideoView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 5/14/26.
//

import SwiftUI
import RTPmacos

struct RemoteVideoView: View {
    
    @State private var accessListsModel: AccessListsModel
    var uploadManagerPtr: OpaquePointer?
    
    init(uploadManagerPtr: OpaquePointer?) {
        accessListsModel = AccessListsModel(uploadManagerPtr: uploadManagerPtr)
        self.uploadManagerPtr = uploadManagerPtr
    }
    
    
    var body: some View {
        List(accessListsModel.accessLists, id: \.namespace_id) { list in
            SharedWithListItemView(users: list.authorized_users, namespace_id: list.namespace_id, uploadManagerPtr: uploadManagerPtr)
                .padding(10)
                .frame(height: 200)
        }
        .onAppear(perform: {
            print("updating")
            accessListsModel.retrieveList()
        })
        .onDisappear(perform: {
            print("deallocating")
            accessListsModel.stopListening()
        })
    }
}

struct AuthorizedUsers: Codable {
    let namespace_id: String
    let authorized_users: [String]
}

@Observable
class AccessListsModel {
    var accessLists: [AuthorizedUsers] = []
    var ptr: UnsafeMutableRawPointer?
    var uploadManagerPtr: OpaquePointer?
    
    init (uploadManagerPtr: OpaquePointer?){
        self.uploadManagerPtr = uploadManagerPtr
    }

    func retrieveList() {
        guard ptr == nil, uploadManagerPtr != nil else { return }
        ptr = Unmanaged.passRetained(self).toOpaque()
        rust_get_shared_videos(uploadManagerPtr, ptr , addAccessList)
    }
    
    func stopListening() {
        guard let p = ptr else { return }
        Unmanaged<AccessListsModel>.fromOpaque(p).release()
        ptr = nil
        accessLists.removeAll()
    }
}


func addAccessList(context: UnsafeMutableRawPointer?, ptr: UnsafePointer<UInt8>?, length: UInt) {
    guard
        let ptr = ptr,
        let context = context
    else { return }
    
    print("now updating UI")
    
    let data = Data(bytes: ptr, count: Int(length))
    
    let accessListModel = Unmanaged<AccessListsModel>.fromOpaque(context).takeUnretainedValue()
    
    do {
        let accessList = try JSONDecoder().decode(AuthorizedUsers.self, from: data)
        DispatchQueue.main.async { [weak accessListModel] in
            accessListModel?.accessLists.append(accessList)
        }
    }
    catch {
        print("Decoding failed: \(error)")
    }
}
