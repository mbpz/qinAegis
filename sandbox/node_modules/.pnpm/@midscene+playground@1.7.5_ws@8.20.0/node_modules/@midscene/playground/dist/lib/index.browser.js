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
    PlaygroundSDK: ()=>index_js_namespaceObject.PlaygroundSDK,
    LocalExecutionAdapter: ()=>local_execution_js_namespaceObject.LocalExecutionAdapter,
    playgroundForSessionManager: ()=>playgroundForSessionManager,
    resolvePreparedLaunchOptions: ()=>external_platform_js_namespaceObject.resolvePreparedLaunchOptions,
    validateStructuredParams: ()=>external_common_js_namespaceObject.validateStructuredParams,
    validationAPIs: ()=>external_common_js_namespaceObject.validationAPIs,
    noReplayAPIs: ()=>external_common_js_namespaceObject.noReplayAPIs,
    BasePlaygroundAdapter: ()=>base_js_namespaceObject.BasePlaygroundAdapter,
    dataExtractionAPIs: ()=>external_common_js_namespaceObject.dataExtractionAPIs,
    PlaygroundServer: ()=>PlaygroundServer,
    createMjpegPreviewDescriptor: ()=>external_platform_js_namespaceObject.createMjpegPreviewDescriptor,
    createScreenshotPreviewDescriptor: ()=>external_platform_js_namespaceObject.createScreenshotPreviewDescriptor,
    RemoteExecutionAdapter: ()=>remote_execution_js_namespaceObject.RemoteExecutionAdapter,
    formatErrorMessage: ()=>external_common_js_namespaceObject.formatErrorMessage,
    definePlaygroundPlatform: ()=>external_platform_js_namespaceObject.definePlaygroundPlatform,
    createScrcpyPreviewDescriptor: ()=>external_platform_js_namespaceObject.createScrcpyPreviewDescriptor,
    launchPreparedPlaygroundPlatform: ()=>launchPreparedPlaygroundPlatform,
    executeAction: ()=>external_common_js_namespaceObject.executeAction,
    playgroundForAgent: ()=>playgroundForAgent,
    playgroundForAgentFactory: ()=>playgroundForAgentFactory
});
const external_common_js_namespaceObject = require("./common.js");
const index_js_namespaceObject = require("./sdk/index.js");
const base_js_namespaceObject = require("./adapters/base.js");
const local_execution_js_namespaceObject = require("./adapters/local-execution.js");
const remote_execution_js_namespaceObject = require("./adapters/remote-execution.js");
const external_platform_js_namespaceObject = require("./platform.js");
const PlaygroundServer = void 0;
const playgroundForAgent = void 0;
const playgroundForAgentFactory = void 0;
const playgroundForSessionManager = void 0;
const launchPreparedPlaygroundPlatform = void 0;
exports.BasePlaygroundAdapter = __webpack_exports__.BasePlaygroundAdapter;
exports.LocalExecutionAdapter = __webpack_exports__.LocalExecutionAdapter;
exports.PlaygroundSDK = __webpack_exports__.PlaygroundSDK;
exports.PlaygroundServer = __webpack_exports__.PlaygroundServer;
exports.RemoteExecutionAdapter = __webpack_exports__.RemoteExecutionAdapter;
exports.createMjpegPreviewDescriptor = __webpack_exports__.createMjpegPreviewDescriptor;
exports.createScrcpyPreviewDescriptor = __webpack_exports__.createScrcpyPreviewDescriptor;
exports.createScreenshotPreviewDescriptor = __webpack_exports__.createScreenshotPreviewDescriptor;
exports.dataExtractionAPIs = __webpack_exports__.dataExtractionAPIs;
exports.definePlaygroundPlatform = __webpack_exports__.definePlaygroundPlatform;
exports.executeAction = __webpack_exports__.executeAction;
exports.formatErrorMessage = __webpack_exports__.formatErrorMessage;
exports.launchPreparedPlaygroundPlatform = __webpack_exports__.launchPreparedPlaygroundPlatform;
exports.noReplayAPIs = __webpack_exports__.noReplayAPIs;
exports.playgroundForAgent = __webpack_exports__.playgroundForAgent;
exports.playgroundForAgentFactory = __webpack_exports__.playgroundForAgentFactory;
exports.playgroundForSessionManager = __webpack_exports__.playgroundForSessionManager;
exports.resolvePreparedLaunchOptions = __webpack_exports__.resolvePreparedLaunchOptions;
exports.validateStructuredParams = __webpack_exports__.validateStructuredParams;
exports.validationAPIs = __webpack_exports__.validationAPIs;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "BasePlaygroundAdapter",
    "LocalExecutionAdapter",
    "PlaygroundSDK",
    "PlaygroundServer",
    "RemoteExecutionAdapter",
    "createMjpegPreviewDescriptor",
    "createScrcpyPreviewDescriptor",
    "createScreenshotPreviewDescriptor",
    "dataExtractionAPIs",
    "definePlaygroundPlatform",
    "executeAction",
    "formatErrorMessage",
    "launchPreparedPlaygroundPlatform",
    "noReplayAPIs",
    "playgroundForAgent",
    "playgroundForAgentFactory",
    "playgroundForSessionManager",
    "resolvePreparedLaunchOptions",
    "validateStructuredParams",
    "validationAPIs"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=index.browser.js.map