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
    getErrorMessage: ()=>getErrorMessage
});
function getErrorMessage(error) {
    if (error instanceof Error) return error.message;
    if (null == error) return String(error);
    if ('object' != typeof error) return String(error);
    const candidate = extractStringMessage(error);
    if (candidate) return candidate;
    try {
        return JSON.stringify(error);
    } catch  {
        return Object.prototype.toString.call(error);
    }
}
function extractStringMessage(error) {
    const anyError = error;
    if ('string' == typeof anyError.message && anyError.message) return anyError.message;
    if (anyError.error && 'string' == typeof anyError.error.message && anyError.error.message) return anyError.error.message;
    if (anyError.cause && 'string' == typeof anyError.cause.message && anyError.cause.message) return anyError.cause.message;
}
exports.getErrorMessage = __webpack_exports__.getErrorMessage;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "getErrorMessage"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});
