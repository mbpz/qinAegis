import type { ElementInfo } from '.';
import type { Point } from '../types';
export declare const getElementXpath: (element: Node, isOrderSensitive?: boolean, isLeafElement?: boolean, limitToCurrentDocument?: boolean) => string;
/** Retrieve XPath for a previously cached node by its hash ID.
 *  Returns a local xpath within the node's own document (limitToCurrentDocument=true). */
export declare function getXpathsById(id: string): string[] | null;
export declare function getXpathsByPoint(point: Point, isOrderSensitive: boolean): string[] | null;
export declare function getNodeInfoByXpath(xpath: string): Node | null;
export declare function getElementInfoByXpath(xpath: string): ElementInfo | null;
