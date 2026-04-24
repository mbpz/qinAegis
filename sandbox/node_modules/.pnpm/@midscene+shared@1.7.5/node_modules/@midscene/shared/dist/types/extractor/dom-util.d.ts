import type { LocateResultElement } from '../types';
export declare function isFormElement(node: globalThis.Node): boolean;
export declare function isButtonElement(node: globalThis.Node): node is globalThis.HTMLButtonElement;
export declare function isAElement(node: globalThis.Node): node is globalThis.HTMLButtonElement;
export declare function isSvgElement(node: globalThis.Node): node is globalThis.SVGSVGElement;
export declare function isImgElement(node: globalThis.Node): node is globalThis.HTMLImageElement;
export declare function isNotContainerElement(node: globalThis.Node): boolean;
export declare function isTextElement(node: globalThis.Node): node is globalThis.HTMLTextAreaElement;
export declare function isContainerElement(node: globalThis.Node): node is globalThis.HTMLElement;
/**
 * Generate a LocateResultElement from a point.
 * This function creates an expanded rect around the given center point.
 *
 * Note: Center coordinates should be integers for pixel-aligned positioning.
 * If decimal values are provided, they will be used as-is, which may result in
 * non-pixel-aligned rect positions.
 *
 * The rect positioning behavior:
 * - When edgeSize is even: center is at the top-left of the four center pixels
 *   For example, with edgeSize=4 and center=[10, 10]:
 *   □□□□
 *   □■□□  (■ represents the center point at pixel 10)
 *   □□□□
 *   □□□□
 *
 * - When edgeSize is odd: center is at the exact middle pixel
 *   For example, with edgeSize=5 and center=[10, 10]:
 *   □□□□□
 *   □□■□□  (■ represents the center point at pixel 10)
 *   □□□□□
 *
 * @param center - Center point coordinates as [x, y] (should be integers)
 * @param description - Description of the element
 * @param edgeSize - Size to expand around the center point (default: 8)
 * @returns A LocateResultElement with rect, center, and description
 */
export declare function generateElementByPoint(center: [number, number], description: string, edgeSize?: number): LocateResultElement;
/**
 * Generate a LocateResultElement from a rect.
 * This function calculates the center point from the rect and preserves the
 * original rect as the returned element boundary.
 *
 * Note: The rect uses inclusive coordinates where:
 * - A rect from [left=10, top=10] with [width=1, height=1] covers exactly 1 pixel
 * - The actual pixel range is [left, left+width) which means width pixels
 *
 * @param sourceRect - The source rect to generate element from (typically contains integer values)
 * @param description - Description of the element
 * @param edgeSize - Deprecated, retained for backward compatibility
 * @returns A LocateResultElement with the original rect, center (always integers), and description
 */
export declare function generateElementByRect(sourceRect: {
    left: number;
    top: number;
    width: number;
    height: number;
}, description: string, _edgeSize?: number): LocateResultElement;
