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
    private var inputConverter: AVAudioConverter?
    private var opus: UnsafeMutablePointer<TsOpus>?
    private var decoders: [UInt16: UnsafeMutablePointer<TsOpus>] = [:]

    private let lock = NSLock()
    /// Encoded packets are FIFO per speaker.  Draining the whole input array
    /// into one mix buffer would overlap consecutive 20 ms packets and make
    /// speech sound accelerated/distorted.
    private var pending: [UInt16: [Data]] = [:]
    private var muted = true
    private let playbackQueue = DispatchQueue(label: "tslib.audio")
    private var playing = false
    private var started = false
    private var playerAttached = false
    private var tapInstalled = false
    private var sentFrames: UInt64 = 0
    private var receivedFrames: UInt64 = 0
    private var unsupportedCodecs = Set<Int>()

    private var captureAccumulator = [Float]()

    var sendFrame: ((Data) -> Void)?
    var onStatus: ((String) -> Void)?
    var onError: ((String) -> Void)?
    var onStats: ((UInt64, UInt64) -> Void)?

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

        // Playback must not depend on microphone permission.  The old code
        // initialized the entire engine only after permission was granted,
        // which also made incoming voices silent when permission was denied.
        DispatchQueue.main.async { [weak self] in
            self?.setupPlaybackAndRequestCapture()
        }
    }

    private func setupPlaybackAndRequestCapture() {
        lock.lock()
        let shouldSetup = started
        lock.unlock()
        guard shouldSetup else { return }

        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.playAndRecord, mode: .voiceChat,
                                    options: [.defaultToSpeaker, .allowBluetooth])
            try session.setPreferredSampleRate(sampleRate)
            try session.setPreferredIOBufferDuration(0.02)
            try session.setActive(true)
        } catch {
            NSLog("TS6: audio session setup failed: \(error)")
            reportError("音频会话启动失败：\(error.localizedDescription)")
        }

        guard let fmt = AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: 1) else {
            reportError("无法创建 48 kHz 播放格式")
            return
        }
        format = fmt

        if !playerAttached {
            engine.attach(player)
            playerAttached = true
        }
        engine.connect(player, to: engine.mainMixerNode, format: fmt)

        engine.prepare()
        do {
            try engine.start()
            player.play()
            reportStatus("扬声器已启动，正在检查麦克风权限…")
        } catch {
            NSLog("TS6: audio engine failed to start: \(error)")
            reportError("扬声器启动失败：\(error.localizedDescription)")
            return
        }
        startPlayback()

        switch session.recordPermission {
        case .granted:
            setupCapture()
        case .denied:
            reportError("麦克风权限被拒绝；可收听但不能发言。请到系统设置中允许麦克风。")
        case .undetermined:
            session.requestRecordPermission { [weak self] granted in
                DispatchQueue.main.async {
                    guard let self = self else { return }
                    if granted {
                        self.setupCapture()
                    } else {
                        self.reportError("麦克风权限被拒绝；可收听但不能发言。请到系统设置中允许麦克风。")
                    }
                }
            }
        @unknown default:
            reportError("无法确定麦克风权限状态")
        }
    }

    private func setupCapture() {
        lock.lock()
        let shouldSetup = started && !tapInstalled
        lock.unlock()
        guard shouldSetup, let format = format else { return }

        do {
            try AVAudioSession.sharedInstance().setActive(true)
        } catch {
            reportError("麦克风音频会话启动失败：\(error.localizedDescription)")
            return
        }

        lock.lock()
        if opus == nil {
            opus = tslib_opus_create(48000, 1, 48000, 20)
        }
        let encoderReady = opus != nil
        lock.unlock()
        guard encoderReady else {
            reportError("Opus 编码器创建失败")
            return
        }

        // Use the node's native format for the tap, then resample to 48 kHz.
        // Installing a 48 kHz tap directly can crash on Bluetooth/44.1 kHz routes.
        let input = engine.inputNode
        let inputFormat = input.inputFormat(forBus: 0)
        guard inputFormat.sampleRate > 0, inputFormat.channelCount > 0 else {
            reportError("当前音频路由没有可用的麦克风输入")
            return
        }
        guard let converter = AVAudioConverter(from: inputFormat, to: format) else {
            reportError("无法把麦克风格式转换为 48 kHz 单声道")
            return
        }
        inputConverter = converter
        input.installTap(onBus: 0, bufferSize: 960, format: inputFormat) { [weak self] buffer, _ in
            self?.capture(buffer)
        }
        lock.lock()
        tapInstalled = true
        lock.unlock()

        // The system permission sheet can interrupt/stop the audio graph on
        // iOS 14. Restart it after permission is granted if necessary.
        if !engine.isRunning {
            engine.prepare()
            do {
                try engine.start()
            } catch {
                reportError("授权后音频引擎重启失败：\(error.localizedDescription)")
                return
            }
        }
        if !player.isPlaying {
            player.play()
        }
        reportStatus("语音已就绪")
    }

    private func capture(_ buffer: AVAudioPCMBuffer) {
        lock.lock()
        defer { lock.unlock() }

        guard started, !muted, let opus = opus else { return }

        let captureBuffer: AVAudioPCMBuffer
        if buffer.format.sampleRate == sampleRate,
           buffer.format.channelCount == 1,
           buffer.format.commonFormat == .pcmFormatFloat32,
           !buffer.format.isInterleaved {
            captureBuffer = buffer
        } else {
            guard let converter = inputConverter else { return }
            let ratio = sampleRate / buffer.format.sampleRate
            let capacity = AVAudioFrameCount(ceil(Double(buffer.frameLength) * ratio) + 32)
            guard let converted = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: capacity) else {
                return
            }
            var supplied = false
            var conversionError: NSError?
            let status = converter.convert(to: converted, error: &conversionError) { _, inputStatus in
                if supplied {
                    inputStatus.pointee = .noDataNow
                    return nil
                }
                supplied = true
                inputStatus.pointee = .haveData
                return buffer
            }
            guard status != .error, conversionError == nil else {
                NSLog("TS6: input conversion failed: \(conversionError?.localizedDescription ?? "unknown")")
                return
            }
            captureBuffer = converted
        }

        guard let channelsPtr = captureBuffer.floatChannelData else { return }

        let channels = Int(captureBuffer.format.channelCount)
        let frames = Int(captureBuffer.frameLength)
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
                sentFrames &+= 1
                if sentFrames % 25 == 0 {
                    reportStatsLocked()
                }
                sendFrame?(Data(out[0..<Int(written)]))
            }
        }
    }

    func handleIncoming(userId: UInt16, codec: Int, data: Data) {
        // Only Opus voice / music is supported.
        guard codec == 4 || codec == 5 else {
            lock.lock()
            let isNew = unsupportedCodecs.insert(codec).inserted
            lock.unlock()
            if isNew {
                reportError("收到不支持的 TS3 语音编码（codec \(codec)）；请把频道编码改为 Opus Voice/Opus Music。")
            }
            return
        }
        lock.lock()
        receivedFrames &+= 1
        pending[userId, default: []].append(data)
        if pending[userId]!.count > 50 {
            pending[userId]!.removeFirst(pending[userId]!.count - 50)
        }
        if receivedFrames % 25 == 0 {
            reportStatsLocked()
        }
        lock.unlock()
    }

    func setMuted(_ muted: Bool) {
        lock.lock()
        self.muted = muted
        let captureReady = tapInstalled && opus != nil
        lock.unlock()
        if !muted && !captureReady {
            reportError("麦克风尚未就绪；请确认系统麦克风权限已开启。")
        }
    }

    /// Caller must hold `lock`.
    private func reportStatsLocked() {
        let sent = sentFrames
        let received = receivedFrames
        DispatchQueue.main.async { [weak self] in self?.onStats?(sent, received) }
    }

    private func reportStatus(_ message: String) {
        DispatchQueue.main.async { [weak self] in self?.onStatus?(message) }
    }

    private func reportError(_ message: String) {
        DispatchQueue.main.async { [weak self] in self?.onError?(message) }
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
            while true {
                self.lock.lock()
                let shouldContinue = self.playing
                self.lock.unlock()
                guard shouldContinue else { break }
                self.processPlayback()
                usleep(20_000)
            }
        }
    }

    private func processPlayback() {
        lock.lock()
        defer { lock.unlock() }

        guard started, !pending.isEmpty, let format = format else { return }

        var mix = [Float](repeating: 0, count: frameSamples)
        var hasData = false

        // Pop at most one packet per speaker per 20 ms mix tick. Packets from
        // different speakers are mixed together; consecutive packets remain
        // ordered for the following ticks.
        let userIds = Array(pending.keys)
        for uid in userIds {
            guard var queue = pending[uid], !queue.isEmpty else { continue }
            let data = queue.removeFirst()
            if queue.isEmpty {
                pending.removeValue(forKey: uid)
            } else {
                pending[uid] = queue
            }
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
        guard engine.isRunning else {
            reportError("收到语音包，但音频引擎已停止。请重新连接服务器。")
            return
        }
        if !player.isPlaying {
            player.play()
        }
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
        muted = true
        let removeTap = tapInstalled
        tapInstalled = false
        lock.unlock()

        // Do not hold the codec lock while stopping AVAudioEngine: stopping can
        // wait for an in-flight tap callback, which also needs this lock.
        engine.stop()
        player.stop()
        if removeTap {
            engine.inputNode.removeTap(onBus: 0)
        }

        lock.lock()
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
        sentFrames = 0
        receivedFrames = 0
        unsupportedCodecs.removeAll()
        inputConverter = nil
        lock.unlock()
    }

    deinit {
        stop()
    }
}
