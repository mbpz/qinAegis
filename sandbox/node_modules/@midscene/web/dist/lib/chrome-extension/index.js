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
    ChromeExtensionProxyPage: ()=>external_page_js_default(),
    overrideAIConfig: ()=>env_namespaceObject.overrideAIConfig,
    ERROR_CODE_NOT_IMPLEMENTED_AS_DESIGNED: ()=>common_namespaceObject.ERROR_CODE_NOT_IMPLEMENTED_AS_DESIGNED,
    ChromeExtensionProxyPageAgent: ()=>external_agent_js_namespaceObject.ChromeExtensionProxyPageAgent
});
const common_namespaceObject = require("@midscene/shared/common");
const external_agent_js_namespaceObject = require("./agent.js");
const external_page_js_namespaceObject = require("./page.js");
var external_page_js_default = /*#__PURE__*/ __webpack_require__.n(external_page_js_namespaceObject);
const env_namespaceObject = require("@midscene/shared/env");
exports.ChromeExtensionProxyPage = __webpack_exports__.ChromeExtensionProxyPage;
exports.ChromeExtensionProxyPageAgent = __webpack_exports__.ChromeExtensionProxyPageAgent;
exports.ERROR_CODE_NOT_IMPLEMENTED_AS_DESIGNED = __webpack_exports__.ERROR_CODE_NOT_IMPLEMENTED_AS_DESIGNED;
exports.overrideAIConfig = __webpack_exports__.overrideAIConfig;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "ChromeExtensionProxyPage",
    "ChromeExtensionProxyPageAgent",
    "ERROR_CODE_NOT_IMPLEMENTED_AS_DESIGNED",
    "overrideAIConfig"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=index.js.map