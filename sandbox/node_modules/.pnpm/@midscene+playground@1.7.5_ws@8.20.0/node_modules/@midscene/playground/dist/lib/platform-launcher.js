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
    launchPreparedPlaygroundPlatform: ()=>launchPreparedPlaygroundPlatform
});
const external_launcher_js_namespaceObject = require("./launcher.js");
const external_platform_js_namespaceObject = require("./platform.js");
async function launchPreparedPlaygroundPlatform(prepared, overrides = {}) {
    const launchOptions = (0, external_platform_js_namespaceObject.resolvePreparedLaunchOptions)(prepared, overrides);
    const applyPreparedPlatform = (result)=>{
        result.server.setPreparedPlatform(prepared);
        return result;
    };
    const startPreparedSidecars = async ()=>{
        if (prepared.sessionManager) return;
        for (const sidecar of prepared.sidecars || [])await sidecar.start();
    };
    if (prepared.agentFactory) {
        await startPreparedSidecars();
        return applyPreparedPlatform(await (0, external_launcher_js_namespaceObject.playgroundForAgentFactory)(prepared.agentFactory).launch(launchOptions));
    }
    if (prepared.agent) {
        await startPreparedSidecars();
        return applyPreparedPlatform(await (0, external_launcher_js_namespaceObject.playgroundForAgent)(prepared.agent).launch(launchOptions));
    }
    if (prepared.sessionManager) return applyPreparedPlatform(await (0, external_launcher_js_namespaceObject.playgroundForSessionManager)().launch(launchOptions));
    throw new Error(`Prepared platform "${prepared.platformId}" must provide agent, agentFactory, or sessionManager`);
}
exports.launchPreparedPlaygroundPlatform = __webpack_exports__.launchPreparedPlaygroundPlatform;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "launchPreparedPlaygroundPlatform"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=platform-launcher.js.map