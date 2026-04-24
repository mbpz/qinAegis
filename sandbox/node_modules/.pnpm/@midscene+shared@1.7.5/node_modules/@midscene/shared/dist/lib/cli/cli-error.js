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
    CLIError: ()=>CLIError,
    reportCLIError: ()=>reportCLIError
});
function _define_property(obj, key, value) {
    if (key in obj) Object.defineProperty(obj, key, {
        value: value,
        enumerable: true,
        configurable: true,
        writable: true
    });
    else obj[key] = value;
    return obj;
}
class CLIError extends Error {
    constructor(message, exitCode = 1){
        super(message), _define_property(this, "exitCode", void 0), this.exitCode = exitCode;
    }
}
function reportCLIError(error, log = console.error) {
    if (error instanceof CLIError) {
        log(error.message);
        return error.exitCode;
    }
    log(error);
    return 1;
}
exports.CLIError = __webpack_exports__.CLIError;
exports.reportCLIError = __webpack_exports__.reportCLIError;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "CLIError",
    "reportCLIError"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});
