//
//  FileBrowser.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 4/13/26.
//
import SwiftUI
import QuickLook
import SwiftUI

struct FileBrowser: View {
    private var files = getFiles() ?? []
    
    @State private var selectedURL: URL?
    @State var exportMenuOpen: Bool = false
    
    var body: some View {
        
        if !files.isEmpty {
            List(files, id: \.self) { file in
                FileRow(url: file, selectedURL: $selectedURL, exportMenuOpen: $exportMenuOpen)
            }
            .quickLookPreview($selectedURL)
            .overlay(content: {
                if exportMenuOpen {
                    Text("YO")
                }
            })
        }
        else {
            Text("No files found.")
        }
    }
}

struct FileRow: View {
    
    var url : URL
    @Binding var selectedURL : URL?
    @Binding var exportMenuOpen: Bool
  
    
    var body: some View {
        HStack {
            Button(action: {
                selectedURL = url
            }) {
                Label(url.lastPathComponent, systemImage: "play.circle")
                    .padding(5)
                    .cornerRadius(10)
            }
            .buttonStyle(PlainButtonStyle())
            
            Spacer()
            
            Button(action: {
                
            }) {
                Label("Share", systemImage: "person.badge.plus")
                    .padding(5)
                    .cornerRadius(10)
            }
            .buttonStyle(PlainButtonStyle())
            
            Button(action: {
                exportMenuOpen = !exportMenuOpen
            }) {
                Label("Export", systemImage: "paperplane.fill")
                    .padding(5)
                    .cornerRadius(10)
            }
            .buttonStyle(PlainButtonStyle())
        }
    }
}

#Preview {
    FileBrowser()
}
