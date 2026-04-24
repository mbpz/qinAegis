import type { Cache, Rect, ReportDumpWithAttributes } from './types';
export { appendFileSync } from 'node:fs';
export declare const groupedActionDumpFileExt = "web-dump.json";
/**
 * Process cache configuration with environment variable support and backward compatibility.
 *
 * @param cache - The original cache configuration
 * @param cacheId - The cache ID to use as:
 *   1. Fallback ID when cache is true or cache object has no ID
 *   2. Legacy cacheId when cache is undefined (requires MIDSCENE_CACHE env var)
 * @returns Processed cache configuration
 */
export declare function processCacheConfig(cache: Cache | undefined, cacheId: string): Cache | undefined;
export declare function getReportTpl(): string;
/**
 * Insert content before </html> in an HTML string.
 * Falls back to simple concatenation if </html> is not found.
 */
export declare function insertContentBeforeClosingHtml(html: string, content: string): string;
/**
 * high performance, insert script before </html> in HTML file
 * only truncate and append, no temporary file
 */
export declare function insertScriptBeforeClosingHtml(filePath: string, scriptContent: string): void;
export declare function reportHTMLContent(dumpData: string | ReportDumpWithAttributes, reportPath?: string, appendReport?: boolean, withTpl?: boolean): string;
export declare function writeDumpReport(fileName: string, dumpData: string | ReportDumpWithAttributes, appendReport?: boolean): string | null;
export declare function writeLogFile(opts: {
    fileName: string;
    fileExt: string;
    fileContent: string | ReportDumpWithAttributes;
    type: 'dump' | 'cache' | 'report' | 'tmp';
    generateReport?: boolean;
    appendReport?: boolean;
}): string | null;
export declare function getTmpDir(): string | null;
export declare function getTmpFile(fileExtWithoutDot: string): string | null;
export declare function overlapped(container: Rect, target: Rect): boolean;
export declare function sleep(ms: number): Promise<unknown>;
export declare function replacerForPageObject(_key: string, value: any): any;
export declare function stringifyDumpData(data: any, indents?: number): string;
export declare function getVersion(): string;
export declare function uploadTestInfoToServer({ testUrl, serverUrl, }: {
    testUrl: string;
    serverUrl?: string;
}): Promise<void>;
