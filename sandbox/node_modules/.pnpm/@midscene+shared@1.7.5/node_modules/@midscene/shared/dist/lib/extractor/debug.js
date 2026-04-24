"use strict";
var __webpack_exports__ = {};
const external_index_js_namespaceObject = require("./index.js");
const external_util_js_namespaceObject = require("./util.js");
console.log((0, external_index_js_namespaceObject.webExtractTextWithPosition)(document.body, true));
console.log(JSON.stringify((0, external_index_js_namespaceObject.webExtractTextWithPosition)(document.body, true)));
(0, external_util_js_namespaceObject.setExtractTextWithPositionOnWindow)();
(0, external_util_js_namespaceObject.setMidsceneVisibleRectOnWindow)();
for(var __rspack_i in __webpack_exports__)exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});
