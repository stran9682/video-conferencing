//
//  AudioConverter.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 3/5/26.
//  Partially based off of:
//  https://nickarner.com/notes/working-with-the-opus-audio-codec-in-swift---august-26-2024/

import Opus
import AVFoundation
import RTPmacos

class AudioManager {
    private var audioEngine: AVAudioEngine!
    private var inputNode: AVAudioInputNode!
    private var encoder: Opus.Encoder?
    
    private var participantNodes: [UInt32: ParticipantAudio] = [:]
    
    private let OPUS_ENCODER_SAMPLE_RATE: Double = 48000
    private let OPUS_ENCODER_DURATION_MS: Int = 20
    private let AUDIO_OUTPUT_SAMPLE_RATE: Double = 48000
    private let AUDIO_OUTPUT_CHANNELS: AVAudioChannelCount = 1
    
    init() {
        do {
            run_runtime_server(StreamType(0))
            rust_send_opus_config(OPUS_ENCODER_SAMPLE_RATE, AUDIO_OUTPUT_CHANNELS)
            
            audioEngine = AVAudioEngine()
            inputNode = audioEngine.inputNode
            
            let inputFormat = AVAudioFormat(standardFormatWithSampleRate: OPUS_ENCODER_SAMPLE_RATE, channels: 1)!
            encoder = try Opus.Encoder(format: inputFormat, application: .voip)
            
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
        guard let encoder = encoder else { return }
        
        do {
            var encodedData = Data(count: Int(buffer.frameLength) * MemoryLayout<Float32>.size)
            _ = try encoder.encode(buffer, to: &encodedData)    // this might be blocking, but anything is better than using AVAudioConverter 🤮
                        
            
            // TODO: Send to RUST
        } catch {
            print("Failed to encode buffer: \(error.localizedDescription)")
        }
    }
    
    // TODO: Make this accessible to RUST and send a pointer to model
    func addParticipant(ssrc: UInt32, sample_rate: Float64, channels: UInt32) -> ParticipantAudio{
        let outputFormat = AVAudioFormat(standardFormatWithSampleRate: sample_rate, channels: channels)!
        
        let participantAudio = ParticipantAudio(outputFormat: outputFormat)
        participantAudio.register(audioEngine: audioEngine, outputFormat: outputFormat)
        
        participantNodes[ssrc] = participantAudio
        
        return participantAudio
    }
}

@_cdecl("swift_receive_audio_config")
public func swift_receive_audio_config(
    _ audio_manager_context: UnsafeMutableRawPointer?,
    _ sample_rate: Double,
    _ channels: UInt32,
    _ ssrc: UInt32
) -> UnsafeMutableRawPointer? {
    guard let audio_manager_context else { return nil }
    
    let audioManager = Unmanaged<AudioManager>.fromOpaque(audio_manager_context).takeUnretainedValue()
    
    let participantAudio = audioManager.addParticipant(ssrc: ssrc, sample_rate: sample_rate, channels: channels)
    
    return Unmanaged.passRetained(participantAudio).toOpaque()
}

class ParticipantAudio {
    private var decoder: Opus.Decoder?
    private var playerNode: AVAudioPlayerNode!
    
    init (outputFormat: AVAudioFormat) {
        do {
            decoder = try Opus.Decoder(format: outputFormat, application: .voip)
            
            playerNode = AVAudioPlayerNode()
        }
        catch {
            fatalError("Failed to create Opus decoder: \(error)")
        }
    }
    
    func register(audioEngine: AVAudioEngine, outputFormat: AVAudioFormat) {
        audioEngine.attach(playerNode)
        audioEngine.connect(playerNode, to: audioEngine.mainMixerNode, format: outputFormat)

        
    }
    
    func play(encodedData: Data) {
        guard let decoder else { return }
        
        playerNode.play()
        
        do {
            let decodedBuffer = try decoder.decode(encodedData)
            
            playerNode.scheduleBuffer(decodedBuffer)
        }
        catch {
            print("Failed to decode buffer: \(error.localizedDescription)")
        }
    }
}
