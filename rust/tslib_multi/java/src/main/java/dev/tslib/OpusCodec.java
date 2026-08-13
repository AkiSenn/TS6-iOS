package dev.tslib;

/**
 * Opus encoder/decoder pair.
 *
 * <p>Not thread-safe.
 *
 * <pre>{@code
 * try (OpusCodec codec = new OpusCodec()) {
 *     byte[] encoded = codec.encode(pcmBytes);
 *     byte[] decoded = codec.decode(encoded);
 * }
 * }</pre>
 */
public class OpusCodec implements AutoCloseable {
    private long nativePtr;

    /**
     * Create an Opus codec with default configuration.
     *
     * @throws TsLibException if creation fails
     */
    public OpusCodec() {
        this(new AudioConfig());
    }

    /**
     * Create an Opus codec with the given configuration.
     *
     * @param config audio configuration
     * @throws TsLibException if creation fails
     */
    public OpusCodec(AudioConfig config) {
        this.nativePtr = nativeCreate(config.sampleRate, config.channels,
                                       config.bitrate, config.frameSizeMs);
        if (this.nativePtr == 0) {
            throw new TsLibException("Failed to create OpusCodec");
        }
    }

    /**
     * Encode PCM samples (16-bit little-endian bytes) to Opus.
     *
     * @param pcm raw PCM data
     * @return Opus-encoded packet
     * @throws TsLibException on encoding error
     */
    public byte[] encode(byte[] pcm) {
        checkNotClosed();
        return nativeEncode(nativePtr, pcm);
    }

    /**
     * Decode an Opus packet to PCM (16-bit little-endian bytes).
     *
     * @param data Opus-encoded data
     * @return decoded PCM samples
     * @throws TsLibException on decoding error
     */
    public byte[] decode(byte[] data) {
        checkNotClosed();
        return nativeDecode(nativePtr, data);
    }

    /**
     * Get the audio configuration used by this codec.
     */
    public AudioConfig getConfig() {
        checkNotClosed();
        return nativeGetConfig(nativePtr);
    }

    @Override
    public void close() {
        if (nativePtr != 0) {
            nativeDestroy(nativePtr);
            nativePtr = 0;
        }
    }

    private void checkNotClosed() {
        if (nativePtr == 0) {
            throw new IllegalStateException("OpusCodec has been closed");
        }
    }

    // Native methods
    private static native long nativeCreate(int sampleRate, int channels,
                                             int bitrate, int frameSizeMs);
    private static native void nativeDestroy(long ptr);
    private static native byte[] nativeEncode(long ptr, byte[] pcm);
    private static native byte[] nativeDecode(long ptr, byte[] data);
    private static native AudioConfig nativeGetConfig(long ptr);
}
