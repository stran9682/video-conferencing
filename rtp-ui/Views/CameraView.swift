//
//  CameraView.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 1/7/26.
//

import SwiftUI

struct CameraView: View {
    
    @Binding var image: CGImage?
    
    var body: some View {
        if let image = image {
            Image(decorative: image, scale: 1)
                .resizable()
                .aspectRatio(contentMode: .fit)
                .frame(minWidth: 0, maxWidth: .infinity, minHeight: 0, maxHeight: .infinity)
        } else {
            ContentUnavailableView("Camera feed interrupted", systemImage: "xmark.circle.fill")
                .frame(minWidth: 0, maxWidth: .infinity, minHeight: 0, maxHeight: .infinity)
        }
    }
}
