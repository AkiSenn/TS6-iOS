import AVFoundation

/// Captures the microphone (48 kHz mono), encodes to Opus via the Rust FFI,
/// and decodes/mixes incoming frames for playback.
///
/// Threading rules:
/// - The capture tap runs on a real-time audio thread; it only does cheap
///   work and fires the (non-blocking) async send into the Rust worker.
/// - All native handle access (opus/decoders) is serialized with `lock` so
///   `stop()` can never race a callback into a use-after-free.
final class AudioBridge {
    private let engine = AVAudioEngine()
    private let player = AVAudioPlayerNode()
    private var format: AVAudioFormat!
    private var opus: UnsafeMutablePointer<TsOpus>?
    private var decoders: [UInt16: UnsafeMutablePointer<TsOpus>] = [:]

    private let lock = NSLock()
    private var pending: [(UInt16, Data)] = []
    private var muted = true
    private let playbackQueue = DispatchQueue(label: "tslib.audio")
    private var playing = false
    private var started = false

    private var captureAccumulator = [Float]()

    var sendFrame: ((Data) -> Void)?

    private let sampleRate = 48000.0
    private let frameSamples = 960 // 20 ms @ 48 kHz

    func start() {
        lock.lock()
        if started {
            lock.unlock()
            return
        }
        started = true
        lock.unlock()

        AVAudioSession.sharedInstance().requestRecordPermission { [weak self] granted in
            guard let self = self else { return }
            guard granted else { return }
            DispatchQueue.main.async { self.setupAudio() }
        }
    }

    private func setupAudio() {
        lock.lock()
        let shouldSetup = started
        lock.unlock()
        guard shouldSetup else { return }

        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.playAndRecord, mode: .voiceChat,
                                    options: [.defaultToSpeaker, .allowBluetooth])
            try session.setPreferredSampleRate(sampleRate)
            try session.setActive(true)
        } catch {
            NSLog("TS6: audio session setup failed: \(error)")
        }

        guard let fmt = AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: 1) else {
            return
        }
        format = fmt

        lock.lock()
        if opus == nil {
            opus = tslib_opus_create(48000, 1, 48000, 20)
        }
        lock.unlock()

        engine.attach(player)
        engine.connect(player, to: engine.mainMixerNode, format: fmt)

        // Use the node's native input format for the tap and convert in the
        // callback; requesting a custom format can trip AVAudioEngine.
        let input = engine.inputNode
        let inputFormat = input.inputFormat(forBus: 0)
        input.installTap(onBus: 0, bufferSize: 4096, format: inputFormat) { [weak self] buffer, _ in
            self?.capture(buffer)
        }

        engine.prepare()
        do {
            try engine.start()
        } catch {
            NSLog("TS6: audio engine failed to start: \(error)")
        }
        startPlayback()
    }

    private func capture(_ buffer: AVAudioPCMBuffer) {
        lock.lock()
        defer { lock.unlock() }

        guard started, !muted, let opus = opus,
              let channelsPtr = buffer.floatChannelData else { return }

        let channels = Int(buffer.format.channelCount)
        let frames = Int(buffer.frameLength)
        guard channels > 0, frames > 0 else { return }

        // Convert the chunk to mono Float and accumulate into whole frames.
        captureAccumulator.reserveCapacity(captureAccumulator.count + frames)
        for i in 0..<frames {
            var v: Float = 0
            for c in 0..<channels {
                v += channelsPtr[c][i]
            }
            captureAccumulator.append(v / Float(channels))
        }

        while captureAccumulator.count >= frameSamples {
            var pcm = [Int16](repeating: 0, count: frameSamples)
            for i in 0..<frameSamples {
                var v = captureAccumulator[i]
                if v > 1 { v = 1 } else if v < -1 { v = -1 }
                pcm[i] = Int16(v * 32767)
            }
            captureAccumulator.removeFirst(frameSamples)

            var out = [UInt8](repeating: 0, count: 2048)
            let written = pcm.withUnsafeBufferPointer { p in
                out.withUnsafeMutableBufferPointer { o in
                    tslib_opus_encode(opus, p.baseAddress, UInt(frameSamples),
                                      o.baseAddress, UInt(o.count))
                }
            }
            if written > 0 {
                sendFrame?(Data(out[0..<Int(written)]))
            }
        }
    }

    func handleIncoming(userId: UInt16, codec: Int, data: Data) {
        // Only Opus voice / music is supported.
        guard codec == 4 || codec == 5 else { return }
        lock.lock()
        pending.append((userId, data))
        if pending.count > 200 {
            pending.removeFirst(50)
        }
        lock.unlock()
    }

    func setMuted(_ muted: Bool) {
        lock.lock()
        self.muted = muted
        lock.unlock()
    }

    private func startPlayback() {
        lock.lock()
        let shouldRun = started && !playing
        if shouldRun {
            playing = true
        }
        lock.unlock()
        guard shouldRun else { return }

        playbackQueue.async { [weak self] in
            guard let self = self else { return }
            while self.playing {
                self.processPlayback()
                usleep(10_000)
            }
        }
    }

    private func processPlayback() {
        lock.lock()
        defer { lock.unlock() }

        guard started, !pending.isEmpty, let format = format else { return }

        let frames = pending
        pending.removeAll()

        var mix = [Float](repeating: 0, count: frameSamples)
        var hasData = false

        for (uid, data) in frames {
            guard let dec = decoderLocked(for: uid) else { continue }
            var pcm = [Int16](repeating: 0, count: frameSamples)
            let capacity = pcm.count
            let samples = data.withUnsafeBytes { raw -> Int32 in
                guard let base = raw.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return pcm.withUnsafeMutableBufferPointer { p in
                    tslib_opus_decode(dec, base, UInt(data.count),
                                      p.baseAddress, UInt(capacity))
                }
            }
            guard samples > 0 else { continue }
            let n = min(Int(samples), frameSamples)
            for i in 0..<n {
                mix[i] += Float(pcm[i]) / 32767.0
            }
            hasData = true
        }

        guard hasData else { return }
        for i in 0..<frameSamples {
            mix[i] = max(-1, min(1, mix[i]))
        }

        let buf = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(frameSamples))!
        buf.frameLength = AVAudioFrameCount(frameSamples)
        mix.withUnsafeBufferPointer { mp in
            guard let dst = buf.floatChannelData?[0] else { return }
            dst.assign(from: mp.baseAddress!, count: frameSamples)
        }
        player.scheduleBuffer(buf)
    }

    /// Caller must hold `lock`.
    private func decoderLocked(for uid: UInt16) -> UnsafeMutablePointer<TsOpus>? {
        if let d = decoders[uid] { return d }
        guard let d = tslib_opus_create(48000, 1, 48000, 20) else { return nil }
        decoders[uid] = d
        return d
    }

    func stop() {
        lock.lock()
        playing = false
        started = false
        engine.stop()
        engine.inputNode.removeTap(onBus: 0)
        if let o = opus {
            tslib_opus_destroy(o)
            opus = nil
        }
        for d in decoders.values {
            tslib_opus_destroy(d)
        }
        decoders.removeAll()
        pending.removeAll()
        captureAccumulator.removeAll()
        lock.unlock()
    }

    deinit {
        stop()
    }
}
