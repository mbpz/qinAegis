"use strict";
var __webpack_require__ = {};
(()=>{
    __webpack_require__.n = (module)=>{
        var getter = module && module.__esModule ? ()=>module['default'] : ()=>module;
        __webpack_require__.d(getter, {
            a: getter
        });
        return getter;
    };
})();
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
    injectWaterFlowAnimation: ()=>injectWaterFlowAnimation,
    getHtmlElementScript: ()=>getHtmlElementScript,
    injectStopWaterFlowAnimation: ()=>injectStopWaterFlowAnimation
});
const external_node_fs_namespaceObject = require("node:fs");
var external_node_fs_default = /*#__PURE__*/ __webpack_require__.n(external_node_fs_namespaceObject);
const utils_namespaceObject = require("@midscene/shared/utils");
let scriptFileContentCache = null;
const getHtmlElementScript = async ()=>{
    const scriptFileToRetrieve = chrome.runtime.getURL("scripts/htmlElement.js");
    if (scriptFileContentCache) return scriptFileContentCache;
    if (utils_namespaceObject.ifInBrowser || utils_namespaceObject.ifInWorker) {
        const script = await fetch(scriptFileToRetrieve);
        scriptFileContentCache = await script.text();
        return scriptFileContentCache;
    }
    return external_node_fs_default().readFileSync(scriptFileToRetrieve, 'utf8');
};
let waterFlowScriptFileContentCache = null;
const injectWaterFlowAnimation = async ()=>{
    const waterFlowScriptFileToRetrieve = chrome.runtime.getURL("scripts/water-flow.js");
    if (waterFlowScriptFileContentCache) return waterFlowScriptFileContentCache;
    if (utils_namespaceObject.ifInBrowser || utils_namespaceObject.ifInWorker) {
        const script = await fetch(waterFlowScriptFileToRetrieve);
        waterFlowScriptFileContentCache = await script.text();
        return waterFlowScriptFileContentCache;
    }
    return external_node_fs_default().readFileSync(waterFlowScriptFileToRetrieve, 'utf8');
};
let stopWaterFlowScriptFileContentCache = null;
const injectStopWaterFlowAnimation = async ()=>{
    const stopWaterFlowScriptFileToRetrieve = chrome.runtime.getURL("scripts/stop-water-flow.js");
    if (stopWaterFlowScriptFileContentCache) return stopWaterFlowScriptFileContentCache;
    if (utils_namespaceObject.ifInBrowser || utils_namespaceObject.ifInWorker) {
        const script = await fetch(stopWaterFlowScriptFileToRetrieve);
        stopWaterFlowScriptFileContentCache = await script.text();
        return stopWaterFlowScriptFileContentCache;
    }
    return external_node_fs_default().readFileSync(stopWaterFlowScriptFileToRetrieve, 'utf8');
};
exports.getHtmlElementScript = __webpack_exports__.getHtmlElementScript;
exports.injectStopWaterFlowAnimation = __webpack_exports__.injectStopWaterFlowAnimation;
exports.injectWaterFlowAnimation = __webpack_exports__.injectWaterFlowAnimation;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "getHtmlElementScript",
    "injectStopWaterFlowAnimation",
    "injectWaterFlowAnimation"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=dynamic-scripts.js.map