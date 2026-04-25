import type { NodeType } from '../constants';
import type { ElementInfo } from '../extractor';
export interface Point {
    left: number;
    top: number;
}
export interface Size {
    width: number;
    height: number;
}
export type Rect = Point & Size;
export declare abstract class BaseElement {
    abstract id: string;
    abstract attributes: {
        nodeType: NodeType;
        [key: string]: string;
    };
    abstract content: string;
    abstract rect: Rect;
    abstract center: [number, number];
    abstract isVisible: boolean;
}
export interface ElementTreeNode<ElementType extends BaseElement = BaseElement> {
    node: ElementType | null;
    children: ElementTreeNode<ElementType>[];
}
export interface WebElementInfo extends ElementInfo {
    zoom: number;
}
export type LocateResultElement = {
    description: string;
    center: [number, number];
    rect: Rect;
};
