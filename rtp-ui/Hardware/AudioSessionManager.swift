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
        
        inputNode.installTap(onBus: 0, bufferSize: desiredBufferSize, format: inputFormat) { [weak self] buffer, when in
            self?.processBuffer(buffer, when: when.hostTime)
        }
        
        rust_send_opus_config(OPUS_ENCODER_SAMPLE_RATE, AUDIO_OUTPUT_CHANNELS)
    }
    
    private func processBuffer(_ buffer: AVAudioPCMBuffer, when: UInt64) {
        guard let encoder = encoder else { return }
        
        do {
            var encodedData = Data(count: Int(buffer.frameLength) * MemoryLayout<Float32>.size)
            _ = try encoder.encode(buffer, to: &encodedData)
                        
            // TODO: Send to RUST
            let pts = AVAudioTime.seconds(forHostTime: when) * 48_000
            let timestamp = UInt32(UInt64(pts) & 0xFFFFFFFF)
            
            rust_send_audio_sample([UInt8](encodedData), UInt(encodedData.count), timestamp)
        } catch {
            print("Failed to encode buffer: \(error.localizedDescription)")
        }
    }
    
    // TODO: Make this accessible to RUST and send a pointer to model
    func addParticipant(ssrc: UInt32, sample_rate: Float64, channels: UInt32) -> ParticipantAudio{
        let outputFormat = AVAudioFormat(standardFormatWithSampleRate: sample_rate, channels: channels)!
        let playerNode = AVAudioPlayerNode()
        
        print("Adding participant — sample_rate: \(sample_rate), channels: \(channels), ssrc: \(ssrc)")
        let participantAudio = ParticipantAudio(outputFormat: outputFormat, playerNode: playerNode)
        
        self.audioEngine.stop()
        
        self.audioEngine.attach(playerNode)
        self.audioEngine.connect(playerNode, to: self.audioEngine.mainMixerNode, format: outputFormat)
        
        playerNode.play()
//        DispatchQueue.main.async {
        self.participantNodes[ssrc] = participantAudio
//        }
        
        self.audioEngine.prepare()
        try? self.audioEngine.start()
        
        return participantAudio
    }
    
    func removeParticipant(ssrc: UInt32) {
        print("Removing participant — ssrc: \(ssrc)")
        
        self.participantNodes[ssrc]?.playerNode.stop()
        self.audioEngine.disconnectNodeOutput(self.participantNodes[ssrc]!.playerNode)
        self.audioEngine.detach(self.participantNodes[ssrc]!.playerNode)
        
//        DispatchQueue.main.async {
        self.participantNodes.removeValue(forKey: ssrc)
//        }
    }
}

@_cdecl("swift_remove_audio_peer")
public func swift_remove_audio_peer(
    _ audioContext: UnsafeMutableRawPointer?,
    _ ssrc: UInt32,
    _ participantContext: UnsafeMutableRawPointer?
) {
    guard let audioContext, let participantContext else { return }
    
    let audioManager = Unmanaged<AudioManager>.fromOpaque(audioContext).takeUnretainedValue()
    
    audioManager.removeParticipant(ssrc: ssrc)
    
    let _ = Unmanaged<ParticipantAudio>.fromOpaque(participantContext).takeRetainedValue()
}

@_cdecl("swift_receive_sample")
public func swift_receive_sample(
    _ context: UnsafeMutableRawPointer?,
    _ audioData: UnsafePointer<UInt8>?,
    _ length: UInt
) {
    guard let context, let audioData else { return }
    
    let participantAudio = Unmanaged<ParticipantAudio>.fromOpaque(context).takeUnretainedValue()
    
    let compressedData = Data(bytes: audioData, count: Int(length))
    
    participantAudio.play(encodedData: compressedData)
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
    public var playerNode: AVAudioPlayerNode
    
    init (outputFormat: AVAudioFormat, playerNode: AVAudioPlayerNode) {
        do {
            decoder = try Opus.Decoder(format: outputFormat, application: .voip)
            
            self.playerNode = playerNode
        }
        catch {
            fatalError("Failed to create Opus decoder: \(error)")
        }
    }
    
    func play(encodedData: Data) {
        guard let decoder else { return }
        
        do {
            let decodedBuffer = try decoder.decode(encodedData)
            
            self.playerNode.scheduleBuffer(decodedBuffer)
            
            if !self.playerNode.isPlaying {
                self.playerNode.play()
            }
        }
        catch {
            print("Failed to decode buffer: \(error.localizedDescription)")
        }
    }
}
