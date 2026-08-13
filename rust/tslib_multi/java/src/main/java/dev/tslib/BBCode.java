package dev.tslib;

/**
 * BBCode parsing utilities.
 *
 * <pre>{@code
 * String html = BBCode.toHtml("[b]Hello[/b]");
 * String plain = BBCode.strip("[color=red]text[/color]");
 * }</pre>
 */
public final class BBCode {
    private BBCode() {}

    /**
     * Convert BBCode markup to HTML.
     */
    public static native String toHtml(String input);

    /**
     * Convert BBCode markup to plain text.
     */
    public static native String toPlain(String input);

    /**
     * Convert BBCode markup to ANSI terminal sequences.
     */
    public static native String toAnsi(String input);

    /**
     * Strip all BBCode tags, returning plain text.
     */
    public static native String strip(String input);
}
