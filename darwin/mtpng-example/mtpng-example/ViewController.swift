//
//  ViewController.swift
//  mtpng-example
//
//  Created by Brooke on 9/19/18.
//  Copyright © 2018-2026 Brooke Vibber. All rights reserved.
//

import UIKit
import MTPNG

private actor SavePNGExample {
    var threads: Int = 0
    var pool: MTPNGThreadPool? = nil
    
    func setThreads(threads: Int) {
        if self.threads == threads && self.pool != nil {
            print("Reusing pool with \(threads) threads")
        } else {
            do {
                pool = try MTPNGThreadPool(threads: threads)
                self.threads = threads
                print("New pool with \(threads) threads")
            } catch {
                self.threads = 0;
                print("Error creating pool with \(threads) threads")
            }
        }
    }

    func savePNGImage(file: String) throws -> TimeInterval {
        let image = UIImage.init(named: file)!
        
        // Draw the UIImage into a CGImage with specified RGB order
        let cgi = image.cgImage!
        
        let width = cgi.width
        let height = cgi.height
        let stride = (cgi.bitsPerPixel / 8) * width
        
        let context = CGContext(data: nil,
                                width: width,
                                height: height,
                                bitsPerComponent: cgi.bitsPerComponent,
                                bytesPerRow: cgi.bytesPerRow,
                                space: cgi.colorSpace!,
                                bitmapInfo: cgi.bitmapInfo)!
        context.draw(cgi, in: CGRect(x: 0, y: 0, width: width, height: height))
        
        // And get the data out.
        let dataSize = height * stride
        let ptr = context.data!.bindMemory(to: UInt8.self, capacity: dataSize)
        let bytes = UnsafeBufferPointer(start: ptr, count: dataSize)
        let data = bytes.span
        
        let options = try MTPNGEncoderOptions()
        try options.setThreadPool(pool: pool!)
        
        let start = Date()
        
        // Create the encoder
        //var outputBuffer: [UInt8] = [];
        let encoder = try MTPNGEncoder.init(
            write: nil /* { (bytes: Span<UInt8>) -> Int in
                bytes.withUnsafeBufferPointer { buffer in
                    outputBuffer.append(contentsOf: buffer)
                }
                return bytes.count;
            } */,
            flush: nil,
            options: options)
        
        let header = try MTPNGHeader()
        try header.setSize(width: UInt32(width), height: UInt32(height))
        try header.setColor(color: MTPNGColor.TruecolorAlpha, bits: 8)
        try encoder.writeHeader(header: header)
        
        try encoder.writeImageRows(bytes: data)
        try encoder.finish()
        
        return Date().timeIntervalSince(start)
    }
}

class ViewController: UIViewController {

    @IBOutlet weak var threadSlider: UISlider!
    @IBOutlet weak var threadLabel: UILabel!
    @IBOutlet weak var samplePicker: UISegmentedControl!
    @IBOutlet weak var compressButton: UIButton!
    @IBOutlet weak var timeLabel: UILabel!
    
    fileprivate var example = SavePNGExample()

    override func viewDidLoad() {
        super.viewDidLoad()
        // Do any additional setup after loading the view, typically from a nib.

        let maxThreads = ProcessInfo.processInfo.processorCount;
        threadSlider.maximumValue = Float(maxThreads)
        threadSlider.minimumValue = 1.0
        threadSlider.value = Float(maxThreads)
        threadLabel.text = String(maxThreads)
        Task {
            await example.setThreads(threads: maxThreads)
        }
    }

    @IBAction func threadsChanged(_ sender: Any) {
        let threads = Int(threadSlider.value)
        threadLabel.text = String(threads)
        Task {
            await example.setThreads(threads: threads)
        }
    }

    @IBAction func samplePickerTouch(_ sender: Any) {
    }

    @IBAction func compressTouch(_ sender: Any) {
        self.timeLabel.text = "Running..."
        let file = self.samplePicker.titleForSegment(at: self.samplePicker.selectedSegmentIndex)!
        Task.detached {
            do {
                let delta = try await self.example.savePNGImage(file: file)
                await self.showResult(delta: delta)
            } catch {
                print("Unexpected error: \(error).")
                await self.showError()
            }
        }
    }
    
    private func showResult(delta: TimeInterval) {
        let ms = Int(delta * 1000.0);
        self.timeLabel.text = String(format: "Done in %d ms.", ms);
    }

    private func showError() {
        self.timeLabel.text = "Error.";
    }
}

