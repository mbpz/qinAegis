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
    setTimingFieldOnce: ()=>setTimingFieldOnce
});
const logger_namespaceObject = require("@midscene/shared/logger");
const debugTiming = (0, logger_namespaceObject.getDebug)('task-timing');
function setTimingFieldOnce(timing, field) {
    if (!timing) return void debugTiming(`[warning] timing object missing, skip set. field=${field}`);
    const value = Date.now();
    const existingValue = timing[field];
    if (void 0 !== existingValue) return void debugTiming(`[warning] duplicate timing field set ignored. field=${field}, existing=${existingValue}, incoming=${value}`);
    timing[field] = value;
}
exports.setTimingFieldOnce = __webpack_exports__.setTimingFieldOnce;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "setTimingFieldOnce"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=task-timing.js.map