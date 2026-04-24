import { descriptionOfTree, treeToList, trimAttributes, truncateText } from "@midscene/shared/extractor";
const ELEMENT_COUNT_WARNING_THRESHOLD = 5000;
const TREE_SIZE_WARNING_MESSAGE = 'The number of elements is too large, it may cause the prompt to be too long, please use domIncluded: "visible-only" to reduce the number of elements';
function tree_descriptionOfTree(tree, truncateTextLength, filterNonTextContent = false, visibleOnly = true) {
    if (!visibleOnly) {
        const flatElements = treeToList(tree);
        if (flatElements.length >= ELEMENT_COUNT_WARNING_THRESHOLD) console.warn(TREE_SIZE_WARNING_MESSAGE);
    }
    return descriptionOfTree(tree, truncateTextLength, filterNonTextContent, visibleOnly);
}
export { tree_descriptionOfTree as descriptionOfTree, trimAttributes, truncateText };

//# sourceMappingURL=tree.mjs.map