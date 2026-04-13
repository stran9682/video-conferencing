//
//  VideoFileManager.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 4/13/26.
//

import Foundation
import AVFoundation

// Source - https://stackoverflow.com/q/65341260
// Posted by spoax, modified by community. See post 'Timeline' for change history
// Retrieved 2026-04-13, License - CC BY-SA 4.0
func getFiles() -> Array<URL>? {
    
    let documentsPath =  FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
    
    do {
        let recordingsPath = documentsPath.appending(path: "/Recordings")
        let directoryContents = try FileManager.default.contentsOfDirectory(at: recordingsPath, includingPropertiesForKeys: nil)
        return directoryContents
    } catch {
        return nil
    }
}
