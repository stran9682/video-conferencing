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
    @State var shareMenuOpen: Bool = false

    var body: some View {
        VStack {
            VStack {
                HStack(alignment: .center) {
                    Button(action: {
                        selectedURL = url
                    }) {
                        Image(systemName: "play.circle")
                            .foregroundStyle(.red)
                    }
                    .buttonStyle(.borderless)

                    Button(action: {
                        shareMenuOpen.toggle()
                    }) {
                        Text(url.lastPathComponent)
                            .foregroundStyle(Color(.white))
                    }
                    .buttonStyle(.borderless)

                    Spacer()

                    Button(action: {
                        exportMenuOpen = !exportMenuOpen
                    }) {
                        Label("Upload", systemImage: "paperplane.fill")
                    }
                    .buttonStyle(.borderless)
                }

                if shareMenuOpen {
                    UserListView()
                        .padding(10)
                        .frame(height: 200)
                }
            }
            .padding(5)

            Divider()
        }
        .listRowSeparator(.hidden)
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
                        rust_upload(
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
