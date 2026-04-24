import type { WebElementInfo } from '../types';
import type { Point } from '../types';
export declare function collectElementInfo(node: Node, currentWindow: typeof window, currentDocument: typeof document, baseZoom?: number, basePoint?: Point, isContainer?: boolean): WebElementInfo | null | any;
interface WebElementNode {
    node: WebElementInfo | null;
    children: WebElementNode[];
}
export declare function extractTextWithPosition(initNode: globalThis.Node, debugMode?: boolean): WebElementInfo[];
export declare function extractTreeNodeAsString(initNode: globalThis.Node, visibleOnly?: boolean, debugMode?: boolean): string;
export declare function extractTreeNode(initNode: globalThis.Node, debugMode?: boolean): WebElementNode;
export declare function mergeElementAndChildrenRects(node: Node, currentWindow: typeof window, currentDocument: typeof document, baseZoom?: number): {
    left: number;
    top: number;
    width: number;
    height: number;
    zoom: number;
    isVisible: boolean;
} | null;
export {};
