package dev.tslib;

/**
 * Audio configuration (immutable).
 */
public class AudioConfig {
    public final int sampleRate;
    public final int channels;
    public final int bitrate;
    public final int frameSizeMs;

    /**
     * Create a default voice-optimized configuration.
     */
    public AudioConfig() {
        this(48000, 1, 32000, 20);
    }

    public AudioConfig(int sampleRate, int channels, int bitrate, int frameSizeMs) {
        this.sampleRate = sampleRate;
        this.channels = channels;
        this.bitrate = bitrate;
        this.frameSizeMs = frameSizeMs;
    }

    /**
     * Configuration optimized for music (stereo, higher bitrate).
     */
    public static AudioConfig music() {
        return new AudioConfig(48000, 2, 96000, 20);
    }

    /**
     * Configuration optimized for low latency.
     */
    public static AudioConfig lowLatency() {
        return new AudioConfig(48000, 1, 32000, 10);
    }

    /**
     * Frame size in samples.
     */
    public int frameSizeSamples() {
        return sampleRate * frameSizeMs / 1000;
    }

    @Override
    public String toString() {
        return "AudioConfig(rate=" + sampleRate + ", ch=" + channels
                + ", bitrate=" + bitrate + ")";
    }
}
