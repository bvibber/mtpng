//
//  ViewController.swift
//  mtpng-example
//
//  Created by Brooke on 9/19/18.
//  Copyright © 2018-2026 Brooke Vibber. All rights reserved.
//

import UIKit

class ViewController: UIViewController {

    @IBOutlet weak var threadSlider: UISlider!
    @IBOutlet weak var threadLabel: UILabel!
    @IBOutlet weak var samplePicker: UISegmentedControl!
    @IBOutlet weak var compressButton: UIButton!
    @IBOutlet weak var timeLabel: UILabel!
    
    var threads: Int = 0;
    var pool: MTPNGThreadPool? = nil;

    func savePngImage(image: UIImage, threads: Int) -> TimeInterval {
        // Draw the UIImage into a CGImage with specified RGB order
        let cgi = image.cgImage!;
        
        let width = cgi.width;
        let height = cgi.height;
        let stride = (cgi.bitsPerPixel / 8) * width;

        let context = CGContext(data: nil,
                                width: width,
                                height: height,
                                bitsPerComponent: cgi.bitsPerComponent,
                                bytesPerRow: cgi.bytesPerRow,
                                space: CGColorSpaceCreateDeviceRGB(),
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue |
                                    CGBitmapInfo.byteOrder32Big.rawValue)!;
        context.draw(cgi, in: CGRect(x: 0, y: 0, width: width, height: height));
        
        // And get the data out.
        let inputPointer = context.data!.assumingMemoryBound(to: UInt8.self);
        let dataSize = height * stride;
        let inputBuffer = UnsafeBufferPointer(start: inputPointer, count: dataSize);
        let data = inputBuffer.span;

        do {
            // Create a manual thread pool
            if self.threads != threads {
                self.threads = threads
                pool = try MTPNGThreadPool.init(threads: threads);
                print("New pool with \(threads) threads")
            } else {
                print("Reusing pool with \(threads) threads")
            }

            let options = try MTPNGEncoderOptions.init();
            try options.setThreadPool(pool: pool!);

            let start = Date();

            // Create the encoder
            var outputBuffer: [UInt8] = [];
            let encoder = try MTPNGEncoder.init(
                write: { (bytes: Span<UInt8>) -> Int in
                    bytes.withUnsafeBufferPointer { buffer in
                        outputBuffer.append(contentsOf: buffer)
                    }
                    return bytes.count;
                },
                flush: nil,
                options: options);

            let header = try MTPNGHeader.init();
            try header.setSize(width: UInt32(width), height: UInt32(height));
            try header.setColor(color: MTPNGColor.TruecolorAlpha, bits: 8);
            try encoder.writeHeader(header: header);

            try encoder.writeImageRows(bytes: data);
            try encoder.finish();

            let delta = Date().timeIntervalSince(start);

            return delta;
        } catch {
            print("Unexpected error: \(error).")
            return 0;
        }
    }
    
    override func viewDidLoad() {
        super.viewDidLoad()
        // Do any additional setup after loading the view, typically from a nib.

        let maxThreads = ProcessInfo.processInfo.processorCount;
        self.threadSlider.maximumValue = Float(maxThreads);
        self.threadSlider.minimumValue = 1.0;
        self.threadSlider.value = Float(maxThreads);
        self.threadLabel.text = String(maxThreads);
    }

    @IBAction func threadsChanged(_ sender: Any) {
        threadLabel.text = String(Int(threadSlider.value));
    }

    @IBAction func samplePickerTouch(_ sender: Any) {
    }

    @IBAction func compressTouch(_ sender: Any) {
        self.timeLabel.text = "Loading...";
        let image = UIImage.init(named: self.samplePicker.titleForSegment(at: self.samplePicker.selectedSegmentIndex)!)!;
        self.timeLabel.text = "Running...";
        let delta = savePngImage(image: image, threads: Int(threadSlider.value));
        let ms = Int(delta * 1000.0);
        self.timeLabel.text = String(format: "Done in %d ms.", ms);
    }
    
}

