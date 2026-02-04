//
//  SharedQueue.swift
//  rtp-ui
//
//  Created by Sebastian Tran on 2/3/26.
//

import Foundation

actor SharedQueue<T> {
    private var elements: [T] = []

    func push(_ item: T) {
        elements.append(item)
    }

    func pop() -> T? {
        guard !elements.isEmpty else { return nil }
        return elements.removeFirst()
    }
}
