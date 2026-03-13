//
//  ViewModel.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/7/26.
//

import Foundation
import CoreImage
import Observation
import RTPmacos

@Observable
class ViewModel {
    var currentFrame: CGImage?
    
    private let cameraManager = CameraManager()
    private let audioManager = AudioManager()
    
    init() {
        audioManager.startRecording()
        rust_send_audio_manger_context(Unmanaged.passUnretained(audioManager).toOpaque())
        
        Task {
            await handleCameraPreviews()
        }
    }
    
    func handleCameraPreviews() async {
        for await image in cameraManager.previewStream {
            Task {
                @MainActor in currentFrame = image
            }
        }
    }
}
