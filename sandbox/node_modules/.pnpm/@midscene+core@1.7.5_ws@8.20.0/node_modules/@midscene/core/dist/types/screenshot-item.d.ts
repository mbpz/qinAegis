import { type ScreenshotRef } from './dump/screenshot-store';
/**
 * Serialization format for ScreenshotItem
 * - { $screenshot: "id" } - inline mode, references imageMap in HTML
 * - { base64: "path" } - directory mode, references external file path
 */
export type ScreenshotSerializeFormat = ScreenshotRef;
/**
 * ScreenshotItem encapsulates screenshot data.
 *
 * Supports lazy loading after memory release:
 * - inline mode: reads from HTML file using streaming (extractImageByIdSync)
 * - directory mode: reads from file on disk
 *
 * After persistence, memory is released but the screenshot can be recovered
 * on-demand from disk, making it safe to release memory at any time.
 */
export declare class ScreenshotItem {
    private _id;
    private _base64;
    private _format;
    private _capturedAt;
    private _serializedRef;
    private _persistedPath;
    private _persistedHtmlPath;
    private constructor();
    /** Create a new ScreenshotItem from base64 data */
    static create(base64: string, capturedAt: number): ScreenshotItem;
    get id(): string;
    /** Get the image format (png or jpeg) */
    get format(): 'png' | 'jpeg';
    /** Get the file extension for this screenshot */
    get extension(): string;
    /** Get screenshot capture timestamp in milliseconds */
    get capturedAt(): number;
    get base64(): string;
    /** Check if base64 data is still available in memory (not yet released) */
    hasBase64(): boolean;
    /**
     * Mark as persisted to HTML (inline mode).
     * Releases base64 memory, but keeps HTML path for lazy loading recovery.
     * @param htmlPath - absolute path to the HTML file containing the image
     */
    markPersistedInline(htmlPath: string): ScreenshotRef;
    /**
     * Register a file-backed recovery path without changing the serialized mode.
     * Used when inline persistence also needs a shared file copy next to dumps.
     */
    registerPersistedFileCopy(relativePath: string, absolutePath: string): ScreenshotRef;
    /**
     * Mark as persisted to file (directory mode).
     * Releases base64 memory, but keeps file path for lazy loading recovery.
     * @param relativePath - relative path for serialization (e.g., "./screenshots/id.jpeg")
     * @param absolutePath - absolute path for lazy loading recovery
     */
    markPersistedToPath(relativePath: string, absolutePath: string): ScreenshotRef;
    /** Serialize for JSON - format depends on persistence state */
    toSerializable(): ScreenshotSerializeFormat;
    /** Check if a value is a serialized ScreenshotItem reference (inline or directory mode) */
    static isSerialized(value: unknown): value is ScreenshotSerializeFormat;
    private createRef;
    /**
     * Get base64 data without the data URI prefix.
     * Useful for writing raw binary data to files.
     */
    get rawBase64(): string;
}
