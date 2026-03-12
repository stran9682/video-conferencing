//
//  AudioConverter.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 3/5/26.
//  Partially based off of:
//  https://nickarner.com/notes/working-with-the-opus-audio-codec-in-swift---august-26-2024/

import Opus
import AVFoundation

class AudioManager {
    private var audioEngine: AVAudioEngine!
    private var inputNode: AVAudioInputNode!
    private var encoder: Opus.Encoder?
    
    private var decoder: Opus.Decoder?
    private var playerNode: AVAudioPlayerNode!
    
    private let OPUS_ENCODER_SAMPLE_RATE: Double = 48000
    private let OPUS_ENCODER_DURATION_MS: Int = 20
    private let AUDIO_OUTPUT_SAMPLE_RATE: Double = 48000
    private let AUDIO_OUTPUT_CHANNELS: AVAudioChannelCount = 1
    
    init() {
        do {
            audioEngine = AVAudioEngine()
            inputNode = audioEngine.inputNode
            
            let inputFormat = AVAudioFormat(standardFormatWithSampleRate: OPUS_ENCODER_SAMPLE_RATE, channels: 1)!
            encoder = try Opus.Encoder(format: inputFormat, application: .voip)
            
            // MARK: Decoding
            let outputFormat = AVAudioFormat(standardFormatWithSampleRate: AUDIO_OUTPUT_SAMPLE_RATE, channels: AUDIO_OUTPUT_CHANNELS)!
            playerNode = AVAudioPlayerNode()
            audioEngine.attach(playerNode)
            decoder = try Opus.Decoder(format: outputFormat, application: .voip)
            
            audioEngine.connect(playerNode, to: audioEngine.mainMixerNode, format: outputFormat)
            
            audioEngine.prepare()
            try audioEngine.start()
            
        }
        catch {
            print("Audio setup error: \(error)")
        }
    }
    
    func startRecording() {
        let inputFormat = AVAudioFormat(standardFormatWithSampleRate: OPUS_ENCODER_SAMPLE_RATE, channels: 1)!
        let desiredBufferSize = AVAudioFrameCount((Double(OPUS_ENCODER_DURATION_MS) / 1000.0) * OPUS_ENCODER_SAMPLE_RATE)
        
        inputNode.installTap(onBus: 0, bufferSize: desiredBufferSize, format: inputFormat) { [weak self] buffer, _ in
            self?.processBuffer(buffer)
        }
    }
    
    private func processBuffer(_ buffer: AVAudioPCMBuffer) {
        guard let encoder = encoder,
              let decoder = decoder
        else { return }
        
        do {
            var encodedData = Data(count: Int(buffer.frameLength) * MemoryLayout<Float32>.size)
            _ = try encoder.encode(buffer, to: &encodedData)
            
            let decodedBuffer = try decoder.decode(encodedData)
            
            playerNode.scheduleBuffer(decodedBuffer)
            
            playerNode.play()
            
            
            // TODO: Send to rust!
        } catch {
            print("Failed to encode buffer: \(error.localizedDescription)")
        }
    }
}
