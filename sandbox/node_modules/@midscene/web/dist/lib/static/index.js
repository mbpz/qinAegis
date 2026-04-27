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
    StaticPageAgent: ()=>external_static_agent_js_namespaceObject.StaticPageAgent,
    StaticPage: ()=>external_static_page_js_default()
});
const external_static_agent_js_namespaceObject = require("./static-agent.js");
const external_static_page_js_namespaceObject = require("./static-page.js");
var external_static_page_js_default = /*#__PURE__*/ __webpack_require__.n(external_static_page_js_namespaceObject);
exports.StaticPage = __webpack_exports__.StaticPage;
exports.StaticPageAgent = __webpack_exports__.StaticPageAgent;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "StaticPage",
    "StaticPageAgent"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=index.js.map