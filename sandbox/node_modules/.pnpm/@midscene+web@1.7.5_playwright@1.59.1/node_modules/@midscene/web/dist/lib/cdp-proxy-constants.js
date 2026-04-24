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
    PROXY_ENDPOINT_FILE: ()=>PROXY_ENDPOINT_FILE,
    TARGET_ID_FILE: ()=>TARGET_ID_FILE,
    PROXY_PID_FILE: ()=>PROXY_PID_FILE,
    PROXY_UPSTREAM_FILE: ()=>PROXY_UPSTREAM_FILE
});
const external_node_os_namespaceObject = require("node:os");
const external_node_path_namespaceObject = require("node:path");
const PROXY_ENDPOINT_FILE = (0, external_node_path_namespaceObject.join)((0, external_node_os_namespaceObject.tmpdir)(), 'midscene-cdp-proxy-endpoint');
const PROXY_PID_FILE = (0, external_node_path_namespaceObject.join)((0, external_node_os_namespaceObject.tmpdir)(), 'midscene-cdp-proxy-pid');
const PROXY_UPSTREAM_FILE = (0, external_node_path_namespaceObject.join)((0, external_node_os_namespaceObject.tmpdir)(), 'midscene-cdp-proxy-upstream');
const TARGET_ID_FILE = (0, external_node_path_namespaceObject.join)((0, external_node_os_namespaceObject.tmpdir)(), 'midscene-cdp-target-id');
exports.PROXY_ENDPOINT_FILE = __webpack_exports__.PROXY_ENDPOINT_FILE;
exports.PROXY_PID_FILE = __webpack_exports__.PROXY_PID_FILE;
exports.PROXY_UPSTREAM_FILE = __webpack_exports__.PROXY_UPSTREAM_FILE;
exports.TARGET_ID_FILE = __webpack_exports__.TARGET_ID_FILE;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "PROXY_ENDPOINT_FILE",
    "PROXY_PID_FILE",
    "PROXY_UPSTREAM_FILE",
    "TARGET_ID_FILE"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=cdp-proxy-constants.js.map