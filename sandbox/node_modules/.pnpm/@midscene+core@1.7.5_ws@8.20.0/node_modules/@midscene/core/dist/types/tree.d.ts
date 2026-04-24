import type { BaseElement, ElementTreeNode } from '@midscene/shared/types';
import { trimAttributes, truncateText } from '@midscene/shared/extractor';
export { trimAttributes, truncateText };
export declare function descriptionOfTree<ElementType extends BaseElement = BaseElement>(tree: ElementTreeNode<ElementType>, truncateTextLength?: number, filterNonTextContent?: boolean, visibleOnly?: boolean): string;
