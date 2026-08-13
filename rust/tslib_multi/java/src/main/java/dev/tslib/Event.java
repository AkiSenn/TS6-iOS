package dev.tslib;

import java.util.Map;

/**
 * A server event (immutable snapshot).
 *
 * <p>The {@link #type} field identifies the event kind (e.g. "connected",
 * "user_joined", "text_message"). The {@link #data} map contains
 * event-specific fields.
 */
public class Event {
    public final String type;
    public final Map<String, Object> data;

    public Event(String type, Map<String, Object> data) {
        this.type = type;
        this.data = data;
    }

    @Override
    public String toString() {
        return "Event(type='" + type + "', data=" + data + ")";
    }
}
