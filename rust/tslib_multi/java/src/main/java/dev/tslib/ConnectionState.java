package dev.tslib;

/**
 * Connection state constants.
 *
 * <p>Compare with {@link Client#getState()}.
 */
public final class ConnectionState {
    public static final int DISCONNECTED = 0;
    public static final int CONNECTING = 1;
    public static final int CONNECTED = 2;
    public static final int INITIALIZING = 3;
    public static final int RECONNECTING = 4;

    private ConnectionState() {}
}
