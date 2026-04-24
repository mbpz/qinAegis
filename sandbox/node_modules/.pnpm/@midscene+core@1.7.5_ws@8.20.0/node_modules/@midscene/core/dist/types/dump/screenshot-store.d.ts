import type { ScreenshotItem } from '../screenshot-item';
export interface ScreenshotRef {
    type: 'midscene_screenshot_ref';
    id: string;
    capturedAt: number;
    mimeType: 'image/png' | 'image/jpeg';
    storage: 'inline' | 'file';
    path?: string;
}
export declare function normalizeScreenshotRef(value: unknown): ScreenshotRef | null;
type ResolvedScreenshotSource = {
    type: 'data-uri';
    id: string;
    mimeType: ScreenshotRef['mimeType'];
    dataUri: string;
} | {
    type: 'file';
    id: string;
    mimeType: ScreenshotRef['mimeType'];
    filePath: string;
};
export declare function resolveScreenshotSource(refInput: unknown, options: {
    reportPath: string;
    fallbackId?: string;
    fallbackMimeType?: ScreenshotRef['mimeType'];
}): ResolvedScreenshotSource;
export declare class ScreenshotStore {
    private readonly mode;
    private readonly reportPath;
    private readonly screenshotsDir?;
    private readonly writeInlineImage?;
    private readonly alsoWriteFileCopy;
    private readonly writtenInlineIds;
    private readonly writtenFileIds;
    constructor(options: {
        mode: 'inline' | 'directory';
        reportPath: string;
        screenshotsDir?: string;
        writeInlineImage?: (id: string, base64: string) => void | Promise<void>;
        alsoWriteFileCopy?: boolean;
        /** @deprecated Use alsoWriteFileCopy instead. */
        ensureFileCopy?: boolean;
    });
    persist(screenshot: ScreenshotItem): Promise<ScreenshotRef>;
    private persistToSharedFileIfNeeded;
    loadBase64(refInput: unknown): string;
    cleanup(): void;
}
export {};
