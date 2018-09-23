//
//  ViewController.swift
//  mtpng-example
//
//  Created by Brion on 9/19/18.
//  Copyright © 2018 Brion Vibber. All rights reserved.
//

import UIKit
import mtpng

func write_func(_user_data: UnsafeMutableRawPointer?, _bytes: UnsafePointer<UInt8>?, _len: Int) -> Int
{
    let myself = Unmanaged<ViewController>.fromOpaque(_user_data!).takeUnretainedValue();
    return myself.writeFunc(bytes: _bytes, len: _len);
}

func flush_func(_user_data: UnsafeMutableRawPointer?) -> Bool
{
    let myself = Unmanaged<ViewController>.fromOpaque(_user_data!).takeUnretainedValue();
    return myself.flushFunc();
}

class ViewController: UIViewController {

    @IBOutlet weak var threadSlider: UISlider!
    @IBOutlet weak var threadLabel: UILabel!
    @IBOutlet weak var samplePicker: UISegmentedControl!
    @IBOutlet weak var compressButton: UIButton!
    @IBOutlet weak var timeLabel: UILabel!
    

    public func writeFunc(bytes: UnsafePointer<UInt8>?, len: Int) -> Int {
        // fake output
        return len;
    }

    public func flushFunc() -> Bool {
        return true;
    }


    func savePngImage(image: UIImage, threads: Int) {
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
        let data = context.data!.assumingMemoryBound(to: UInt8.self);

        // Create a manual thread pool
        var pool = OpaquePointer.init(bitPattern: 0);
        let user_data = UnsafeMutableRawPointer.init(Unmanaged.passUnretained(self).toOpaque());

        var ret = mtpng_threadpool_new(&pool, threads);
        if (ret != MTPNG_RESULT_OK) {
            NSLog("Failure!");
        }

        // Set it in the options
        var options = OpaquePointer.init(bitPattern: 0);
        ret = mtpng_encoder_options_new(&options);
        if (ret != MTPNG_RESULT_OK) {
            NSLog("Failure!");
        }

        ret = mtpng_encoder_options_set_thread_pool(options, pool);
        if (ret != MTPNG_RESULT_OK) {
            NSLog("Failure!");
        }

        // Create the encoder
        var encoder = OpaquePointer.init(bitPattern: 0);
        ret = mtpng_encoder_new(&encoder,
                          write_func,
                          flush_func,
                          user_data,
                          options);
        if (ret != MTPNG_RESULT_OK) {
            NSLog("Failure!");
        }

        var header = OpaquePointer.init(bitPattern: 0);
        ret = mtpng_header_new(&header);
        if (ret != MTPNG_RESULT_OK) {
            NSLog("Failure!");
        }

        ret = mtpng_header_set_size(header, UInt32(width), UInt32(height));
        if (ret != MTPNG_RESULT_OK) {
            NSLog("Failure!");
        }

        ret = mtpng_header_set_color(header, MTPNG_COLOR_TRUECOLOR_ALPHA, 8);
        if (ret != MTPNG_RESULT_OK) {
            NSLog("Failure!");
        }

        ret = mtpng_encoder_write_header(encoder, header);
        if (ret != MTPNG_RESULT_OK) {
            NSLog("Failure!");
        }

        // @fixme for some reason this crashes:
        //mtpng_encoder_write_image_rows(encoder, &(data[0]), stride * height);
        // have to do row by row instead:
        for y in 0..<height {
            ret = mtpng_encoder_write_image_rows(encoder, &(data[y * stride]), stride);
            if (ret != MTPNG_RESULT_OK) {
                NSLog("Failure!");
                return;
            }
        }
        ret = mtpng_encoder_finish(&encoder);
        if (ret != MTPNG_RESULT_OK) {
            NSLog("Failure!");
        }

        ret = mtpng_header_release(&header);
        if (ret != MTPNG_RESULT_OK) {
            NSLog("Failure!");
        }

        ret = mtpng_encoder_options_release(&options);
        if (ret != MTPNG_RESULT_OK) {
            NSLog("Failure!");
        }

        ret = mtpng_threadpool_release(&pool);
        if (ret != MTPNG_RESULT_OK) {
            NSLog("Failure!");
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
        let start = Date();
        savePngImage(image: image, threads: Int(threadSlider.value));
        let delta = Date().timeIntervalSince(start);
        let ms = Int(delta * 1000.0);
        self.timeLabel.text = String(format: "Done in %d ms.", ms);
    }
    
}

