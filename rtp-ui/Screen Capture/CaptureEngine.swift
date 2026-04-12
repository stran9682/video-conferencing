//
//  CaptureEngine.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 3/28/26.
//
import Foundation
import ScreenCaptureKit


class CaptureEngine: NSObject {
    var audioEngine: AVAudioEngine = AVAudioEngine()
    var micInput: AVAudioPlayerNode = AVAudioPlayerNode()
    var systemInput: AVAudioPlayerNode = AVAudioPlayerNode()
    var mixer: AVAudioMixerNode = AVAudioMixerNode()
    
    private var hlsWriter: HLSWriter?
    
    private var stream: SCStream?
    private let videoSampleBufferQueue = DispatchQueue(label: "com.example.apple-samplecode.VideoSampleBufferQueue")
    private let audioSampleBufferQueue = DispatchQueue(label: "com.example.apple-samplecode.AudioSampleBufferQueue")
    private let micSampleBufferQueue = DispatchQueue(label: "com.example.apple-samplecode.MicSampleBufferQueue")
        
    func startCapture(configuration: SCStreamConfiguration, filter: SCContentFilter) async {
        guard stream == nil,
              hlsWriter == nil
        else { return }
        
        let format = AVAudioFormat(standardFormatWithSampleRate: 44100, channels: 2)!
        
        do {
            stream = SCStream(filter: filter, configuration: configuration, delegate: self)
            hlsWriter = HLSWriter()
            hlsWriter?.startWriting()
            
            audioEngine.attach(micInput)
            audioEngine.attach(systemInput)
            audioEngine.attach(mixer)
            
            if configuration.captureMicrophone {
                try stream?.addStreamOutput(self, type: .microphone, sampleHandlerQueue: micSampleBufferQueue)
                audioEngine.connect(micInput, to: mixer, format: format)
            }
            
            if configuration.capturesAudio {
                try stream?.addStreamOutput(self, type: .audio, sampleHandlerQueue: audioSampleBufferQueue)
                audioEngine.connect(systemInput, to: mixer, format: format)
            }
            
            if configuration.captureMicrophone || configuration.capturesAudio {
                mixer.installTap(onBus: 0, bufferSize: 1024, format: mixer.outputFormat(forBus: 0), block: { [weak self] buffer, time in
                   // Convert buffer into CMSampleBuffer and send to HLS writer
                    guard let cmbuffer = Converter.configureSampleBuffer(pcmBuffer: buffer, time: time) else { return }
                    
                    do {
                        try self?.hlsWriter?.appendAudio(cmbuffer)
                    }
                    catch {
                        print("Error occured while appending audio to hls: \(error)")
                    }
                })
                audioEngine.connect(mixer, to: audioEngine.mainMixerNode, format: format)
                audioEngine.mainMixerNode.outputVolume = 0
                
                audioEngine.prepare()
                try audioEngine.start()
            }

            
            try stream?.addStreamOutput(self, type: .screen, sampleHandlerQueue: videoSampleBufferQueue)
            
            try await stream?.startCapture()
        } catch {
            print("Failed to start stream: \(error)")
        }
    }
    
    func stopCapture() async {
        guard let stream = self.stream,
              let hlsWriter = self.hlsWriter
        else { return }
        
        audioEngine.stop()
        audioEngine.reset()
        
        await hlsWriter.finishWriting()
    
        do {
            try await stream.stopCapture()
        } catch {
            print("Failed to stop stream: \(error)")
        }
        
        self.stream = nil
        self.hlsWriter = nil
    }
}

/// A class that handles output from an SCStream, and handles stream errors.
extension CaptureEngine: SCStreamOutput, SCStreamDelegate {
        
    func stream(_
                stream: SCStream,
                didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
                of outputType: SCStreamOutputType
    ) {
        guard sampleBuffer.isValid,
              let hlsWriter = hlsWriter
        else { return }
        
        // Determine which type of data the sample buffer contains.
        switch outputType {
        case .screen:
            do {
                try hlsWriter.appendVideo(sampleBuffer)
            }
            catch {
                print("Error occured while appending video to hls: \(error)")
            }
        case .audio:
            let buffer = AVAudioPCMBuffer.create(from: sampleBuffer)!
            systemInput.scheduleBuffer(buffer)
            if !systemInput.isPlaying { systemInput.play() }
        case .microphone:
           let buffer = AVAudioPCMBuffer.create(from: sampleBuffer)!
            micInput.scheduleBuffer(buffer)
            if !micInput.isPlaying { micInput.play() }
        @unknown default:
            print("Unknown output type: \(outputType)")
        }
        
    }
    
    func stream(_ stream: SCStream, didStopWithError error: Error) {
        // Perhaps something here?
        print(error)
    }
}

// Source - https://stackoverflow.com/a/67599278
// Posted by Niko, modified by community. See post 'Timeline' for change history
// Retrieved 2026-04-12, License - CC BY-SA 4.0
extension AVAudioPCMBuffer {
    static func create(from sampleBuffer: CMSampleBuffer) -> AVAudioPCMBuffer? {
    
        guard let description: CMFormatDescription = CMSampleBufferGetFormatDescription(sampleBuffer),
              let sampleRate: Float64 = description.audioStreamBasicDescription?.mSampleRate,
              let channelsPerFrame: UInt32 = description.audioStreamBasicDescription?.mChannelsPerFrame /*,
         let numberOfChannels = description.audioChannelLayout?.numberOfChannels */
        else { return nil }
        
        guard let blockBuffer: CMBlockBuffer = CMSampleBufferGetDataBuffer(sampleBuffer) else {
            return nil
        }
        
        let samplesCount = CMSampleBufferGetNumSamples(sampleBuffer)
        
        //let length: Int = CMBlockBufferGetDataLength(blockBuffer)
        
        let audioFormat = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: sampleRate, channels: AVAudioChannelCount(2), interleaved: false)
        
        let buffer = AVAudioPCMBuffer(pcmFormat: audioFormat!, frameCapacity: AVAudioFrameCount(samplesCount))!
        buffer.frameLength = buffer.frameCapacity
        
        // GET BYTES
        var dataPointer: UnsafeMutablePointer<Int8>?
        CMBlockBufferGetDataPointer(blockBuffer, atOffset: 0, lengthAtOffsetOut: nil, totalLengthOut: nil, dataPointerOut: &dataPointer)
        
        guard var channel: UnsafeMutablePointer<Float> = buffer.floatChannelData?[0],
              let data = dataPointer else { return nil }
        
        var data16 = UnsafeRawPointer(data).assumingMemoryBound(to: Int16.self)
        
        for _ in 0...samplesCount - 1 {
            channel.pointee = Float32(data16.pointee) / Float32(Int16.max)
            channel += 1
            for _ in 0...channelsPerFrame - 1 {
                data16 += 1
            }
            
        }
    
        return buffer
    }
}

class Converter {
    static func configureSampleBuffer(pcmBuffer: AVAudioPCMBuffer, time: AVAudioTime) -> CMSampleBuffer? {
        let audioBufferList = pcmBuffer.mutableAudioBufferList
        let asbd = pcmBuffer.format.streamDescription

        var sampleBuffer: CMSampleBuffer? = nil
        var format: CMFormatDescription? = nil
        
        var status = CMAudioFormatDescriptionCreate(allocator: kCFAllocatorDefault,
                                                         asbd: asbd,
                                                   layoutSize: 0,
                                                       layout: nil,
                                                       magicCookieSize: 0,
                                                       magicCookie: nil,
                                                       extensions: nil,
                                                       formatDescriptionOut: &format);
        if (status != noErr) { return nil; }
        
        var timing: CMSampleTimingInfo = CMSampleTimingInfo(duration: CMTime(value: 1, timescale: Int32(asbd.pointee.mSampleRate)),
                                                            presentationTimeStamp: CMTime(value: CMTimeValue(time.sampleTime), timescale: CMTimeScale(time.sampleRate)),
                                                            decodeTimeStamp: CMTime.invalid)
        status = CMSampleBufferCreate(allocator: kCFAllocatorDefault,
                                      dataBuffer: nil,
                                      dataReady: false,
                                      makeDataReadyCallback: nil,
                                      refcon: nil,
                                      formatDescription: format,
                                      sampleCount: CMItemCount(pcmBuffer.frameLength),
                                      sampleTimingEntryCount: 1,
                                      sampleTimingArray: &timing,
                                      sampleSizeEntryCount: 0,
                                      sampleSizeArray: nil,
                                      sampleBufferOut: &sampleBuffer);
        if (status != noErr) { NSLog("CMSampleBufferCreate returned error: \(status)"); return nil }
        
        status = CMSampleBufferSetDataBufferFromAudioBufferList(sampleBuffer!,
                                                                blockBufferAllocator: kCFAllocatorDefault,
                                                                blockBufferMemoryAllocator: kCFAllocatorDefault,
                                                                flags: 0,
                                                                bufferList: audioBufferList);
        if (status != noErr) { NSLog("CMSampleBufferSetDataBufferFromAudioBufferList returned error: \(status)"); return nil; }
        
        return sampleBuffer
    }
}
