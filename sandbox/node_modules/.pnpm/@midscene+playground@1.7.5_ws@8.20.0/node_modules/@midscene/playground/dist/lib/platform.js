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
    createMjpegPreviewDescriptor: ()=>createMjpegPreviewDescriptor,
    createScrcpyPreviewDescriptor: ()=>createScrcpyPreviewDescriptor,
    createScreenshotPreviewDescriptor: ()=>createScreenshotPreviewDescriptor,
    definePlaygroundPlatform: ()=>definePlaygroundPlatform,
    resolvePreparedLaunchOptions: ()=>resolvePreparedLaunchOptions
});
function definePlaygroundPlatform(descriptor) {
    return descriptor;
}
function createScreenshotPreviewDescriptor(overrides = {}) {
    return {
        kind: 'screenshot',
        screenshotPath: '/screenshot',
        capabilities: [
            {
                kind: 'screenshot',
                label: 'Screenshot polling',
                live: false
            }
        ],
        ...overrides
    };
}
function createMjpegPreviewDescriptor(overrides = {}) {
    return {
        kind: 'mjpeg',
        screenshotPath: '/screenshot',
        mjpegPath: '/mjpeg',
        capabilities: [
            {
                kind: 'mjpeg',
                label: 'MJPEG streaming',
                live: true
            },
            {
                kind: 'screenshot',
                label: 'Screenshot fallback',
                live: false
            }
        ],
        ...overrides
    };
}
function createScrcpyPreviewDescriptor(custom = {}, overrides = {}) {
    return {
        kind: 'scrcpy',
        screenshotPath: '/screenshot',
        capabilities: [
            {
                kind: 'scrcpy',
                label: 'scrcpy streaming',
                live: true
            },
            {
                kind: 'screenshot',
                label: 'Screenshot fallback',
                live: false
            }
        ],
        custom,
        ...overrides
    };
}
function resolvePreparedLaunchOptions(prepared, overrides = {}) {
    return {
        ...prepared.launchOptions || {},
        ...overrides
    };
}
exports.createMjpegPreviewDescriptor = __webpack_exports__.createMjpegPreviewDescriptor;
exports.createScrcpyPreviewDescriptor = __webpack_exports__.createScrcpyPreviewDescriptor;
exports.createScreenshotPreviewDescriptor = __webpack_exports__.createScreenshotPreviewDescriptor;
exports.definePlaygroundPlatform = __webpack_exports__.definePlaygroundPlatform;
exports.resolvePreparedLaunchOptions = __webpack_exports__.resolvePreparedLaunchOptions;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "createMjpegPreviewDescriptor",
    "createScrcpyPreviewDescriptor",
    "createScreenshotPreviewDescriptor",
    "definePlaygroundPlatform",
    "resolvePreparedLaunchOptions"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=platform.js.map