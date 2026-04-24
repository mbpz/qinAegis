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
    overrideAIConfig: ()=>env_namespaceObject.overrideAIConfig,
    AgentOverChromeBridge: ()=>external_agent_cli_side_js_namespaceObject.AgentOverChromeBridge,
    killRunningServer: ()=>external_io_server_js_namespaceObject.killRunningServer
});
const external_agent_cli_side_js_namespaceObject = require("./agent-cli-side.js");
const env_namespaceObject = require("@midscene/shared/env");
const external_io_server_js_namespaceObject = require("./io-server.js");
exports.AgentOverChromeBridge = __webpack_exports__.AgentOverChromeBridge;
exports.killRunningServer = __webpack_exports__.killRunningServer;
exports.overrideAIConfig = __webpack_exports__.overrideAIConfig;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "AgentOverChromeBridge",
    "killRunningServer",
    "overrideAIConfig"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=index.js.map