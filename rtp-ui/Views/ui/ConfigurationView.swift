//
//  ConfigurationView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 3/28/26.
//

import ScreenCaptureKit
import SwiftUI

struct ConfigurationView: View {
    private let sectionSpacing: CGFloat = 20
    private let verticalLabelSpacing: CGFloat = 8

    private let alignmentOffset: CGFloat = 10

    @Bindable var screenRecorder: ScreenRecorder
    @State var showPickerSettingsView = false
    @State private var isRecordingActive = false

    var body: some View {
        VStack {
            Form {
                HeaderView("Video")
                    .padding(EdgeInsets(top: 0, leading: 0, bottom: 1, trailing: 0))

                Group {
                    VStack(alignment: .leading, spacing: verticalLabelSpacing) {
                        Text("Capture Type")
                        Picker("Capture", selection: $screenRecorder.captureType) {
                            Text("Display")
                                .tag(ScreenRecorder.CaptureType.display)
                            Text("Window")
                                .tag(ScreenRecorder.CaptureType.window)
                        }
                    }

                    VStack(alignment: .leading, spacing: verticalLabelSpacing) {
                        Text("Screen Content")
                        switch screenRecorder.captureType {
                        case .display:
                            Picker("Display", selection: $screenRecorder.selectedDisplay) {
                                ForEach(screenRecorder.availableDisplays, id: \.self) { display in
                                    Text(display.displayName)
                                        .tag(SCDisplay?.some(display))
                                }
                            }

                        case .window:
                            Picker("Window", selection: $screenRecorder.selectedWindow) {
                                ForEach(screenRecorder.availableWindows, id: \.self) { window in
                                    Text(window.displayName)
                                        .tag(SCWindow?.some(window))
                                }
                            }
                        }
                    }
                }
                .labelsHidden()

                Toggle("Exclude sample app from stream", isOn: $screenRecorder.isAppExcluded)
                    .disabled(screenRecorder.captureType == .window)

                // Add some space between the Video and Audio sections.
                Spacer()
                    .frame(height: 20)

                HeaderView("Audio")

                Toggle("Add mic output", isOn: $screenRecorder.isMicCaptureEnabled)
                Toggle("Capture audio", isOn: $screenRecorder.isAudioCaptureEnabled)
                Toggle("Exclude app audio", isOn: $screenRecorder.isAppAudioExcluded)
                    .disabled(screenRecorder.isAppExcluded)
            }
            .padding()

            Spacer()
            HStack {
                Button {
                    Task {
                        await screenRecorder.start()
                    }
                } label: {
                    Text("Start Capture")
                }
                .disabled(screenRecorder.isRunning)
                Button {
                    Task {
                        await screenRecorder.stop()
                    }
                } label: {
                    Text("Stop Capture")
                }
                .disabled(!screenRecorder.isRunning)
            }
            .frame(maxWidth: .infinity, minHeight: 60)
        }
        .background(Color(red: 42 / 255, green: 41 / 255, blue: 48 / 255))
    }
}

/// A view that displays a styled header for the Video and Audio sections.
struct HeaderView: View {
    private let title: String
    private let alignmentOffset: CGFloat = 10.0

    init(_ title: String) {
        self.title = title
    }

    var body: some View {
        Text(title)
            .font(.headline)
            .foregroundColor(.secondary)
            .alignmentGuide(.leading) { _ in alignmentOffset }
    }
}

#Preview {
    var screenRecorder = ScreenRecorder()

    ConfigurationView(screenRecorder: screenRecorder)
        .frame(minWidth: 280, maxWidth: 280)
}
