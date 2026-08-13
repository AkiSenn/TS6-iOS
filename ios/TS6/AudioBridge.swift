import AVFoundation

/// Captures the microphone (48 kHz mono), encodes to Opus via the Rust FFI,
/// and decodes/mixes incoming frames for playback.
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

    var sendFrame: ((Data) -> Void)?

    private let sampleRate = 48000.0
    private let frameSamples = 960 // 20 ms @ 48 kHz

    func start() {
        guard !started else { return }
        started = true
        AVAudioSession.sharedInstance().requestRecordPermission { [weak self] granted in
            guard let self = self else { return }
            if granted {
                DispatchQueue.main.async { self.setupAudio() }
            }
        }
    }

    private func setupAudio() {
        let session = AVAudioSession.sharedInstance()
        try? session.setCategory(.playAndRecord, mode: .voiceChat,
                                 options: [.defaultToSpeaker, .allowBluetooth])
        try? session.setPreferredSampleRate(sampleRate)
        try? session.setActive(true)

        format = AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: 1)
        opus = tslib_opus_create(48000, 1, 48000, 20)

        engine.attach(player)
        engine.connect(player, to: engine.mainMixerNode, format: format)

        let input = engine.inputNode
        input.installTap(onBus: 0, bufferSize: AVAudioFrameCount(frameSamples),
                         format: format) { [weak self] buffer, _ in
            self?.capture(buffer)
        }

        engine.prepare()
        try? engine.start()
        startPlayback()
    }

    private func capture(_ buffer: AVAudioPCMBuffer) {
        guard !muted, let opus = opus, let ch = buffer.floatChannelData?[0] else { return }
        let frames = Int(buffer.frameLength)
        guard frames >= frameSamples else { return }

        var pcm = [Int16](repeating: 0, count: frameSamples)
        for i in 0..<frameSamples {
            var v = ch[i]
            if v > 1 { v = 1 } else if v < -1 { v = -1 }
            pcm[i] = Int16(v * 32767)
        }

        var out = [UInt8](repeating: 0, count: 2048)
        let written = pcm.withUnsafeBufferPointer { p in
            out.withUnsafeMutableBufferPointer { o in
                tslib_opus_encode(opus, p.baseAddress, UInt(frameSamples),
                                  o.baseAddress, UInt(o.count))
            }
        }
        guard written > 0 else { return }
        sendFrame?(Data(out[0..<Int(written)]))
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
        self.muted = muted
    }

    private func startPlayback() {
        guard !playing else { return }
        playing = true
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
        let frames = pending
        pending.removeAll()
        lock.unlock()

        guard !frames.isEmpty, let format = format else { return }

        var mix = [Float](repeating: 0, count: frameSamples)
        var hasData = false

        for (uid, data) in frames {
            guard let dec = decoder(for: uid) else { continue }
            var pcm = [Int16](repeating: 0, count: frameSamples)
            let samples = data.withUnsafeBytes { raw -> Int32 in
                guard let base = raw.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return pcm.withUnsafeMutableBufferPointer { p in
                    tslib_opus_decode(dec, base, UInt(data.count),
                                      p.baseAddress, UInt(pcm.count))
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

    private func decoder(for uid: UInt16) -> UnsafeMutablePointer<TsOpus>? {
        if let d = decoders[uid] { return d }
        guard let d = tslib_opus_create(48000, 1, 48000, 20) else { return nil }
        decoders[uid] = d
        return d
    }

    func stop() {
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
        lock.lock()
        pending.removeAll()
        lock.unlock()
    }

    deinit {
        stop()
    }
}
