import AVFoundation

/// Fixed-size PCM jitter buffer. All access is protected by `pcmLock`.
private final class PCMQueue {
    private var storage: [Float]
    private var readIndex = 0
    private var writeIndex = 0
    private(set) var count = 0
    private var primed = false
    private let primeFrames: Int

    init(capacity: Int, primeFrames: Int) {
        storage = [Float](repeating: 0, count: capacity)
        self.primeFrames = primeFrames
    }

    func write(_ samples: [Float]) {
        for sample in samples {
            if count == storage.count {
                // Bound latency by discarding the oldest sample on overflow.
                readIndex = (readIndex + 1) % storage.count
                count -= 1
            }
            storage[writeIndex] = sample
            writeIndex = (writeIndex + 1) % storage.count
            count += 1
        }
    }

    @discardableResult
    func mix(into output: UnsafeMutablePointer<Float>, frameCount: Int) -> Bool {
        if !primed {
            guard count >= max(primeFrames, frameCount) else { return false }
            primed = true
        }
        guard count >= frameCount else {
            // Rebuffer after underrun instead of playing broken fragments.
            primed = false
            return false
        }
        for i in 0..<frameCount {
            output[i] += storage[readIndex]
            readIndex = (readIndex + 1) % storage.count
            count -= 1
        }
        return true
    }
}

private struct DecoderState {
    let pointer: UnsafeMutablePointer<TsOpus>
    let channels: Int
}

/// AVAudioEngine capture plus pull-based, low-latency playback.
///
/// Incoming Opus is decoded on a serial queue into a bounded 60 ms jitter
/// buffer. AVAudioSourceNode pulls exactly the PCM frame count requested by
/// the hardware. This avoids timer drift and accumulated scheduling delay.
final class AudioBridge {
    private let engine = AVAudioEngine()
    private var sourceNode: AVAudioSourceNode?
    private var sourceAttached = false
    private var format: AVAudioFormat!
    private var inputConverter: AVAudioConverter?
    private var opus: UnsafeMutablePointer<TsOpus>?

    /// State/capture lock. The render callback never takes this lock.
    private let lock = NSLock()
    private var muted = true
    private var started = false
    private var tapInstalled = false
    private var captureAccumulator = [Float]()
    private var sentFrames: UInt64 = 0
    private var receivedFrames: UInt64 = 0
    private var unsupportedCodecs = Set<Int>()
    private var restartScheduled = false
    private var notificationTokens: [NSObjectProtocol] = []
    private var localTalking = false
    private var quietCaptureFrames = 0

    /// Decoder handles live only on this queue. Opus decode therefore never
    /// blocks the real-time render callback.
    private let decoderQueue = DispatchQueue(label: "tslib.audio.decode", qos: .userInteractive)
    private var decoders: [UInt16: DecoderState] = [:]

    /// The render callback holds this only while copying decoded PCM.
    private let pcmLock = NSLock()
    private var pcmQueues: [UInt16: PCMQueue] = [:]

    /// Returns whether the frame entered the native send queue.
    var sendFrame: ((Data) -> Bool)?
    var onStatus: ((String) -> Void)?
    var onError: ((String) -> Void)?
    var onStats: ((UInt64, UInt64) -> Void)?
    var onLocalTalk: ((Bool) -> Void)?

    private let sampleRate = 48000.0
    private let frameSamples = 960        // 20 ms capture frames
    private let maxDecodeFrames = 5760   // Opus maximum: 120 ms @ 48 kHz
    private let jitterPrimeFrames = 2880 // 60 ms
    private let jitterCapacity = 11520   // 240 ms hard latency cap

    init() {
        let center = NotificationCenter.default
        let interruption = center.addObserver(
            forName: AVAudioSession.interruptionNotification,
            object: AVAudioSession.sharedInstance(),
            queue: .main
        ) { [weak self] note in
            guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
                  let type = AVAudioSession.InterruptionType(rawValue: raw) else { return }
            if type == .ended {
                self?.scheduleEngineRestart()
            }
        }
        let routeChange = center.addObserver(
            forName: AVAudioSession.routeChangeNotification,
            object: AVAudioSession.sharedInstance(),
            queue: .main
        ) { [weak self] _ in
            self?.scheduleEngineRestart()
        }
        notificationTokens = [interruption, routeChange]
    }

    func start() {
        lock.lock()
        if started {
            lock.unlock()
            return
        }
        started = true
        lock.unlock()

        // Playback is initialized regardless of microphone permission.
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
            if session.maximumInputNumberOfChannels > 0 {
                try session.setPreferredInputNumberOfChannels(1)
            }
            try session.setActive(true)
        } catch {
            reportError("音频会话启动失败：\(error.localizedDescription)")
        }

        guard let fmt = AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: 1) else {
            reportError("无法创建 48 kHz 播放格式")
            return
        }
        format = fmt

        if sourceNode == nil {
            sourceNode = AVAudioSourceNode(format: fmt) { [weak self] _, _, frameCount, audioList in
                self?.render(frameCount: Int(frameCount), audioList: audioList) ?? noErr
            }
        }
        if let sourceNode = sourceNode {
            if !sourceAttached {
                engine.attach(sourceNode)
                sourceAttached = true
            }
            engine.connect(sourceNode, to: engine.mainMixerNode, format: fmt)
        }

        switch session.recordPermission {
        case .granted:
            if !setupCapture() {
                startPlaybackOnly()
            }
        case .denied:
            startPlaybackOnly()
            reportError("麦克风权限被拒绝；可收听但不能发言。请到系统设置中允许麦克风。")
        case .undetermined:
            startPlaybackOnly()
            session.requestRecordPermission { [weak self] granted in
                DispatchQueue.main.async {
                    guard let self = self else { return }
                    if granted {
                        _ = self.setupCapture()
                    } else {
                        self.reportError("麦克风权限被拒绝；可收听但不能发言。请到系统设置中允许麦克风。")
                    }
                }
            }
        @unknown default:
            reportError("无法确定麦克风权限状态")
        }
    }

    private func startPlaybackOnly() {
        guard !engine.isRunning else { return }
        engine.prepare()
        do {
            try engine.start()
            reportStatus("扬声器已启动")
        } catch {
            reportError("扬声器启动失败：\(error.localizedDescription)")
        }
    }

    @discardableResult
    private func setupCapture() -> Bool {
        lock.lock()
        let isStarted = started
        let alreadyReady = tapInstalled && opus != nil
        lock.unlock()
        guard isStarted else { return false }
        if alreadyReady {
            if !engine.isRunning { scheduleEngineRestart() }
            return true
        }
        guard let format = format else {
            setCaptureFailure("音频格式尚未建立")
            return false
        }

        do {
            try AVAudioSession.sharedInstance().setActive(true)
        } catch {
            setCaptureFailure("音频会话启动失败：\(error.localizedDescription)")
            return false
        }

        lock.lock()
        if opus == nil {
            opus = tslib_opus_create(48000, 1, 48000, 20)
        }
        let encoderReady = opus != nil
        lock.unlock()
        guard encoderReady else {
            setCaptureFailure("Opus 编码器创建失败")
            return false
        }

        let input = engine.inputNode
        let inputFormat = input.outputFormat(forBus: 0)
        guard inputFormat.sampleRate > 0, inputFormat.channelCount > 0 else {
            setCaptureFailure("当前音频路由没有可用的输入")
            return false
        }
        guard let converter = AVAudioConverter(from: inputFormat, to: format) else {
            setCaptureFailure("无法转换麦克风音频格式")
            return false
        }
        inputConverter = converter

        if engine.isRunning {
            engine.stop()
        }
        input.installTap(onBus: 0, bufferSize: 960, format: nil) { [weak self] buffer, _ in
            self?.capture(buffer)
        }
        lock.lock()
        tapInstalled = true
        lock.unlock()

        engine.prepare()
        do {
            try engine.start()
        } catch {
            setCaptureFailure("音频引擎启动失败：\(error.localizedDescription)")
            return false
        }
        reportStatus("语音已就绪 · 低延迟播放")
        return true
    }

    private func setCaptureFailure(_ message: String) {
        reportError("麦克风未就绪：\(message)")
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
            guard status != .error, conversionError == nil else { return }
            captureBuffer = converted
        }

        guard let channelsPointer = captureBuffer.floatChannelData else { return }
        let channels = Int(captureBuffer.format.channelCount)
        let frames = Int(captureBuffer.frameLength)
        guard channels > 0, frames > 0 else { return }

        captureAccumulator.reserveCapacity(captureAccumulator.count + frames)
        var energy: Float = 0
        for i in 0..<frames {
            var value: Float = 0
            for channel in 0..<channels {
                value += channelsPointer[channel][i]
            }
            value /= Float(channels)
            energy += value * value
            captureAccumulator.append(value)
        }
        updateLocalTalkLocked(rms: sqrt(energy / Float(frames)), frames: frames)

        while captureAccumulator.count >= frameSamples {
            var pcm = [Int16](repeating: 0, count: frameSamples)
            for i in 0..<frameSamples {
                let value = max(-1, min(1, captureAccumulator[i]))
                pcm[i] = Int16(value * 32767)
            }
            captureAccumulator.removeFirst(frameSamples)

            var output = [UInt8](repeating: 0, count: 2048)
            let written = pcm.withUnsafeBufferPointer { pcmPointer in
                output.withUnsafeMutableBufferPointer { outputPointer in
                    tslib_opus_encode(opus, pcmPointer.baseAddress, UInt(frameSamples),
                                      outputPointer.baseAddress, UInt(outputPointer.count))
                }
            }
            if written > 0 {
                let queued = sendFrame?(Data(output[0..<Int(written)])) ?? false
                if queued {
                    sentFrames &+= 1
                    if sentFrames % 25 == 0 { reportStatsLocked() }
                } else {
                    reportError("麦克风已编码，但语音帧未进入发送队列")
                }
            }
        }
    }

    func handleIncoming(userId: UInt16, codec: Int, data: Data) {
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
        guard started else {
            lock.unlock()
            return
        }
        receivedFrames &+= 1
        if receivedFrames % 25 == 0 { reportStatsLocked() }
        lock.unlock()

        if !engine.isRunning {
            scheduleEngineRestart()
        }

        decoderQueue.async { [weak self] in
            self?.decodeIncoming(userId: userId, codec: codec, data: data)
        }
    }

    private func decodeIncoming(userId: UInt16, codec: Int, data: Data) {
        lock.lock()
        let isStarted = started
        lock.unlock()
        guard isStarted else { return }

        // Opus Voice is mono; Opus Music is conventionally stereo in TS3.
        let channels = codec == 5 ? 2 : 1
        let decoder: DecoderState
        if let existing = decoders[userId], existing.channels == channels {
            decoder = existing
        } else {
            if let existing = decoders.removeValue(forKey: userId) {
                tslib_opus_destroy(existing.pointer)
            }
            guard let pointer = tslib_opus_create(48000, Int32(channels), 48000, 20) else {
                reportError("无法为用户 \(userId) 创建 Opus 解码器")
                return
            }
            decoder = DecoderState(pointer: pointer, channels: channels)
            decoders[userId] = decoder
        }

        var pcm = [Int16](repeating: 0, count: maxDecodeFrames * channels)
        let decoded = data.withUnsafeBytes { raw -> Int32 in
            guard let base = raw.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return pcm.withUnsafeMutableBufferPointer { pointer in
                tslib_opus_decode(decoder.pointer, base, UInt(data.count),
                                  pointer.baseAddress, UInt(pointer.count))
            }
        }
        guard decoded > 0 else {
            reportError("Opus 解码失败（用户 \(userId)，数据 \(data.count) 字节）")
            return
        }

        let totalSamples = min(Int(decoded), pcm.count)
        let outputFrames = totalSamples / channels
        var mono = [Float](repeating: 0, count: outputFrames)
        if channels == 1 {
            for i in 0..<outputFrames {
                mono[i] = Float(pcm[i]) / 32767.0
            }
        } else {
            for i in 0..<outputFrames {
                let left = Float(pcm[i * 2])
                let right = Float(pcm[i * 2 + 1])
                mono[i] = (left + right) / 65534.0
            }
        }

        pcmLock.lock()
        let queue = pcmQueues[userId] ?? PCMQueue(capacity: jitterCapacity,
                                                  primeFrames: jitterPrimeFrames)
        pcmQueues[userId] = queue
        queue.write(mono)
        pcmLock.unlock()
    }

    private func render(frameCount: Int,
                        audioList: UnsafeMutablePointer<AudioBufferList>) -> OSStatus {
        let buffers = UnsafeMutableAudioBufferListPointer(audioList)
        guard buffers.count > 0, let firstData = buffers[0].mData else { return noErr }
        let output = firstData.assumingMemoryBound(to: Float.self)
        output.initialize(repeating: 0, count: frameCount)

        pcmLock.lock()
        var mixedQueues = 0
        for queue in pcmQueues.values {
            if queue.mix(into: output, frameCount: frameCount) {
                mixedQueues += 1
            }
        }
        pcmLock.unlock()

        // Avoid hard clipping when several users speak at once.
        let gain: Float = mixedQueues > 1 ? 1.0 / sqrt(Float(mixedQueues)) : 1.0
        for i in 0..<frameCount {
            output[i] = max(-1, min(1, output[i] * gain))
        }
        return noErr
    }

    private func scheduleEngineRestart() {
        lock.lock()
        guard started, !restartScheduled else {
            lock.unlock()
            return
        }
        restartScheduled = true
        lock.unlock()

        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.lock.lock()
            let shouldRestart = self.started
            self.restartScheduled = false
            self.lock.unlock()
            guard shouldRestart, !self.engine.isRunning else { return }

            do {
                try AVAudioSession.sharedInstance().setActive(true)
                self.engine.prepare()
                try self.engine.start()
                self.reportStatus("音频引擎已自动恢复")
            } catch {
                self.reportError("音频引擎恢复失败：\(error.localizedDescription)")
            }
        }
    }

    func setMuted(_ muted: Bool) {
        lock.lock()
        self.muted = muted
        let captureReady = tapInstalled && opus != nil
        let wasTalking = localTalking
        if muted {
            localTalking = false
            quietCaptureFrames = 0
        }
        lock.unlock()
        if muted && wasTalking {
            DispatchQueue.main.async { [weak self] in self?.onLocalTalk?(false) }
        }
        if !muted && !captureReady {
            reportStatus("正在重新初始化麦克风…")
            DispatchQueue.main.async { [weak self] in
                _ = self?.setupCapture()
            }
        }
    }

    private func updateLocalTalkLocked(rms: Float, frames: Int) {
        if rms >= 0.012 {
            quietCaptureFrames = 0
            if !localTalking {
                localTalking = true
                DispatchQueue.main.async { [weak self] in self?.onLocalTalk?(true) }
            }
        } else if localTalking {
            quietCaptureFrames += frames
            if quietCaptureFrames >= 16_800 {
                localTalking = false
                quietCaptureFrames = 0
                DispatchQueue.main.async { [weak self] in self?.onLocalTalk?(false) }
            }
        }
    }

    func prepareForBackground() {
        do {
            try AVAudioSession.sharedInstance().setActive(true)
        } catch {
            reportError("后台音频会话启动失败：\(error.localizedDescription)")
        }
        if !engine.isRunning { scheduleEngineRestart() }
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

    func stop() {
        lock.lock()
        started = false
        muted = true
        let removeTap = tapInstalled
        tapInstalled = false
        lock.unlock()

        engine.stop()
        if removeTap {
            engine.inputNode.removeTap(onBus: 0)
        }

        lock.lock()
        if let encoder = opus {
            tslib_opus_destroy(encoder)
            opus = nil
        }
        captureAccumulator.removeAll()
        let wasTalking = localTalking
        localTalking = false
        quietCaptureFrames = 0
        sentFrames = 0
        receivedFrames = 0
        unsupportedCodecs.removeAll()
        restartScheduled = false
        inputConverter = nil
        lock.unlock()

        if wasTalking {
            DispatchQueue.main.async { [weak self] in self?.onLocalTalk?(false) }
        }

        decoderQueue.sync {
            for decoder in decoders.values {
                tslib_opus_destroy(decoder.pointer)
            }
            decoders.removeAll()
        }
        pcmLock.lock()
        pcmQueues.removeAll()
        pcmLock.unlock()
    }

    deinit {
        for token in notificationTokens {
            NotificationCenter.default.removeObserver(token)
        }
        stop()
    }
}
