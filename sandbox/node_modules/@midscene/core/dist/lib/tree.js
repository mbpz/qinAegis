"use strict";
var __webpack_require__ = {};
(()=>{
    __webpack_require__.d = (exports1, definition)=>{
        for(var key in definition)if (__webpack_require__.o(definition, key) && !__webpack_require__.o(exports1, key)) Object.defineProperty(exports1, key, {
            enumerable: true,
            get: definition[key]
        });
    };
})();
(()=>{
    __webpack_require__.o = (obj, prop)=>Object.prototype.hasOwnProperty.call(obj, prop);
})();
(()=>{
    __webpack_require__.r = (exports1)=>{
        if ('undefined' != typeof Symbol && Symbol.toStringTag) Object.defineProperty(exports1, Symbol.toStringTag, {
            value: 'Module'
        });
        Object.defineProperty(exports1, '__esModule', {
            value: true
        });
    };
})();
var __webpack_exports__ = {};
__webpack_require__.r(__webpack_exports__);
__webpack_require__.d(__webpack_exports__, {
    descriptionOfTree: ()=>descriptionOfTree,
    truncateText: ()=>extractor_namespaceObject.truncateText,
    trimAttributes: ()=>extractor_namespaceObject.trimAttributes
});
const extractor_namespaceObject = require("@midscene/shared/extractor");
const ELEMENT_COUNT_WARNING_THRESHOLD = 5000;
const TREE_SIZE_WARNING_MESSAGE = 'The number of elements is too large, it may cause the prompt to be too long, please use domIncluded: "visible-only" to reduce the number of elements';
function descriptionOfTree(tree, truncateTextLength, filterNonTextContent = false, visibleOnly = true) {
    if (!visibleOnly) {
        const flatElements = (0, extractor_namespaceObject.treeToList)(tree);
        if (flatElements.length >= ELEMENT_COUNT_WARNING_THRESHOLD) console.warn(TREE_SIZE_WARNING_MESSAGE);
    }
    return (0, extractor_namespaceObject.descriptionOfTree)(tree, truncateTextLength, filterNonTextContent, visibleOnly);
}
exports.descriptionOfTree = __webpack_exports__.descriptionOfTree;
exports.trimAttributes = __webpack_exports__.trimAttributes;
exports.truncateText = __webpack_exports__.truncateText;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "descriptionOfTree",
    "trimAttributes",
    "truncateText"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=tree.js.map