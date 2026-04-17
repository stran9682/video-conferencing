//
//  FileBrowser.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 4/13/26.
//
import SwiftUI
import QuickLook
import SwiftUI
import RTPmacos

struct FileBrowser: View {
    private var files = getFiles() ?? []
    
    @State private var selectedURL: URL?
    
    var body: some View {
        
        if !files.isEmpty {
            List(files, id: \.self) { file in
                FileRow(url: file, selectedURL: $selectedURL)
            }
            .quickLookPreview($selectedURL)
        }
        else {
            Text("No files found.")
        }
    }
}

struct FileRow: View {
    
    var url : URL
    @Binding var selectedURL : URL?
    @State var exportMenuOpen: Bool = false
    @State var endpointAddress: String = ""
  
    
    var body: some View {
        VStack {
            HStack(alignment: .top) {
                Button(action: {
                    selectedURL = url
                }) {
                    Label(url.lastPathComponent, systemImage: "play.circle")
                        .padding(5)
                }
                .buttonStyle(PlainButtonStyle())
                
                Spacer()
                
                Button(action: {
                    exportMenuOpen = !exportMenuOpen
                }) {
                    Label("Upload", systemImage: "paperplane.fill")
                        .padding(5)
                        .cornerRadius(10)
                }
                .buttonStyle(PlainButtonStyle())
            }
            
            if exportMenuOpen {
                HStack {
                    TextField("Enter Endpoint", text: $endpointAddress)
                        .textFieldStyle(.roundedBorder)
                        .frame(maxWidth: 200)
                    
                    Button("Upload") {
//                        swift_upload(
//                            selectedURL!.absoluteString,
//                            UInt(selectedURL!.absoluteString.count),
//                            endpointAddress,
//                            UInt(endpointAddress.count)
//                        )
                    }
                }
            }

        }
    }
}

#Preview {
    FileBrowser()
}
