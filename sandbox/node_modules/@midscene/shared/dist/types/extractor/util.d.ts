import type { Rect } from '../types';
export declare function setDebugMode(mode: boolean): void;
export declare function getDebugMode(): boolean;
export declare function logger(..._msg: any[]): void;
export declare function isElementPartiallyInViewport(rect: ReturnType<typeof getRect>, currentWindow: typeof window, currentDocument: typeof document, visibleAreaRatio?: number): boolean;
export declare function getPseudoElementContent(element: globalThis.Node, currentWindow: typeof globalThis.window): {
    before: string;
    after: string;
};
export declare function hasOverflowY(element: globalThis.HTMLElement, currentWindow: typeof globalThis.window): boolean;
export interface ExtractedRect {
    width: number;
    height: number;
    left: number;
    top: number;
    right: number;
    bottom: number;
    x: number;
    y: number;
    zoom: number;
}
export declare function overlappedRect(rect1: ExtractedRect, rect2: ExtractedRect): ExtractedRect | null;
export declare function getRect(el: globalThis.HTMLElement | globalThis.Node, baseZoom: number, // base zoom
currentWindow: typeof globalThis.window): ExtractedRect;
export declare function elementRect(el: globalThis.HTMLElement | globalThis.Node | null, currentWindow: typeof globalThis.window, currentDocument: typeof globalThis.document, baseZoom?: number): {
    left: number;
    top: number;
    width: number;
    height: number;
    zoom: number;
    isVisible: boolean;
} | false;
export declare function validTextNodeContent(node: globalThis.Node): string | false;
export declare function getNodeAttributes(node: globalThis.HTMLElement | globalThis.Node, currentWindow: typeof globalThis.window): Record<string, string>;
/**
 * Reset the node hash cache. Call at the beginning of each extraction cycle
 * to prevent stale DOM references from accumulating.
 */
export declare function setNodeHashCacheListOnWindow(): void;
export declare function setNodeToCacheList(node: globalThis.Node, id: string): void;
export declare function getNodeFromCacheList(id: string): globalThis.Node | undefined;
export declare function midsceneGenerateHash(node: globalThis.Node | null, content: string, rect: Rect): string;
export declare function generateId(numberId: number): string;
export declare function setGenerateHashOnWindow(): void;
export declare function setMidsceneVisibleRectOnWindow(): void;
export declare function setExtractTextWithPositionOnWindow(): void;
export declare function getTopDocument(): globalThis.HTMLElement;
