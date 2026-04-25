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
    resolvePreviewDescriptor: ()=>resolvePreviewDescriptor,
    normalizeExecutionUxHints: ()=>normalizeExecutionUxHints,
    buildRuntimeInfo: ()=>buildRuntimeInfo
});
const external_platform_js_namespaceObject = require("./platform.js");
function normalizeExecutionUxHints(metadata) {
    if (!metadata) return [];
    const fromHints = metadata.executionUxHints;
    if (Array.isArray(fromHints)) return fromHints.filter((value)=>'string' == typeof value && value.length > 0);
    const fromSingle = metadata.executionUx;
    if ('string' == typeof fromSingle && fromSingle.length > 0) return [
        fromSingle
    ];
    return [];
}
function resolvePreviewDescriptor(input) {
    if (input.preview) return input.preview;
    if ('number' == typeof input.scrcpyPort) return (0, external_platform_js_namespaceObject.createScrcpyPreviewDescriptor)({
        scrcpyPort: input.scrcpyPort
    });
    if (input.mjpegStreamUrl) return (0, external_platform_js_namespaceObject.createMjpegPreviewDescriptor)();
    if (input.supportsScreenshot) return (0, external_platform_js_namespaceObject.createScreenshotPreviewDescriptor)();
    return {
        kind: 'none',
        capabilities: []
    };
}
function buildRuntimeInfo(input) {
    const interfaceType = input.interfaceType || 'Unknown';
    return {
        platformId: input.platformId,
        title: input.title,
        platformDescription: input.platformDescription,
        interface: {
            type: interfaceType,
            description: input.interfaceDescription
        },
        preview: resolvePreviewDescriptor(input),
        executionUxHints: normalizeExecutionUxHints(input.metadata),
        metadata: {
            ...input.metadata || {}
        }
    };
}
exports.buildRuntimeInfo = __webpack_exports__.buildRuntimeInfo;
exports.normalizeExecutionUxHints = __webpack_exports__.normalizeExecutionUxHints;
exports.resolvePreviewDescriptor = __webpack_exports__.resolvePreviewDescriptor;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "buildRuntimeInfo",
    "normalizeExecutionUxHints",
    "resolvePreviewDescriptor"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=runtime-metadata.js.map