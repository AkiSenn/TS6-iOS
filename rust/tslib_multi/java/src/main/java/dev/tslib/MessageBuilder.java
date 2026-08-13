package dev.tslib;

/**
 * A fluent builder for BBCode-formatted messages.
 *
 * <p>Implemented entirely in Java (no JNI required).
 *
 * <pre>{@code
 * String msg = new MessageBuilder()
 *     .bold("Hello")
 *     .text(" ")
 *     .italic("world")
 *     .build();
 * }</pre>
 */
public class MessageBuilder {
    private final StringBuilder content = new StringBuilder();

    public MessageBuilder text(String t) {
        content.append(t);
        return this;
    }

    public MessageBuilder bold(String t) {
        content.append("[b]").append(t).append("[/b]");
        return this;
    }

    public MessageBuilder italic(String t) {
        content.append("[i]").append(t).append("[/i]");
        return this;
    }

    public MessageBuilder underline(String t) {
        content.append("[u]").append(t).append("[/u]");
        return this;
    }

    public MessageBuilder color(String color, String t) {
        content.append("[color=").append(color).append("]")
               .append(t).append("[/color]");
        return this;
    }

    public MessageBuilder url(String url, String text) {
        content.append("[url=").append(url).append("]")
               .append(text).append("[/url]");
        return this;
    }

    public MessageBuilder newline() {
        content.append("\n");
        return this;
    }

    public String build() {
        return content.toString();
    }

    @Override
    public String toString() {
        return "MessageBuilder(len=" + content.length() + ")";
    }
}
