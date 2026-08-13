package dev.tslib;

/**
 * Exception thrown by tslib native methods.
 */
public class TsLibException extends RuntimeException {
    public TsLibException(String message) {
        super(message);
    }
}
