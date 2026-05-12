//
//  FileBrowser.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 4/13/26.
//
import QuickLook
import RTPmacos
import SwiftData
import SwiftUI

struct FileBrowser: View {
    private var files = getFiles() ?? []

    @State private var selectedURL: URL?

    var body: some View {
        if !files.isEmpty {
            List(files, id: \.self) { file in
                FileRow(url: file, selectedURL: $selectedURL)
            }
            .quickLookPreview($selectedURL)
        } else {
            Text("No files found.")
        }
    }
}

struct FileRow: View {
    var url: URL
    @Binding var selectedURL: URL?
    @State var exportMenuOpen: Bool = false

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
        }
        .sheet(isPresented: $exportMenuOpen, content: {
            uploadView(url: url)

        })
    }
}

struct uploadView: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.modelContext) private var modelContext

    @State private var endpointAddress: String = ""
    let url: URL

    var body: some View {
        NavigationStack {
            HStack {
                TextField("Enter Endpoint", text: $endpointAddress)
                    .textFieldStyle(.roundedBorder)
                    .padding(10)
            }
            .toolbar(content: {
                ToolbarItem(placement: .cancellationAction, content: {
                    Button(role: .cancel) {
                        dismiss()
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundStyle(.gray)
                            .symbolVariant(.circle.fill)
                    }
                })

                ToolbarItem(placement: .confirmationAction, content: {
                    Button("Upload") {
                        swift_upload(
                            url.relativePath,
                            UInt(url.relativePath.count),
                            endpointAddress,
                            UInt(endpointAddress.count)
                        )
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(.green)
                })
            })
        }
    }
}

#Preview {
    FileBrowser()
}
