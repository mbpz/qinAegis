import type { BaseElement, ElementTreeNode } from '../types';
export declare function truncateText(text: string | number | object | undefined, maxLength?: number): string;
export declare function trimAttributes(attributes: Record<string, any>, truncateTextLength?: number): Record<string, string>;
export declare function descriptionOfTree<ElementType extends BaseElement = BaseElement>(tree: ElementTreeNode<ElementType>, truncateTextLength?: number, filterNonTextContent?: boolean, visibleOnly?: boolean): string;
export declare function treeToList<T extends BaseElement>(tree: ElementTreeNode<T>): T[];
export declare function traverseTree<T extends BaseElement, ReturnNodeType extends BaseElement>(tree: ElementTreeNode<T>, onNode: (node: T) => ReturnNodeType): ElementTreeNode<ReturnNodeType>;
