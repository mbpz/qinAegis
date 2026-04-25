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
    camelToKebab: ()=>camelToKebab,
    getKeyAliases: ()=>getKeyAliases,
    isRecord: ()=>isRecord,
    kebabToCamel: ()=>kebabToCamel
});
function kebabToCamel(str) {
    return str.replace(/-([a-z])/g, (_, letter)=>letter.toUpperCase());
}
function camelToKebab(str) {
    return str.replace(/[A-Z]/g, (letter)=>`-${letter.toLowerCase()}`).replace(/^-/, '');
}
function getKeyAliases(key) {
    return [
        ...new Set([
            key,
            kebabToCamel(key),
            camelToKebab(key)
        ])
    ];
}
function isRecord(value) {
    return 'object' == typeof value && null !== value && !Array.isArray(value);
}
exports.camelToKebab = __webpack_exports__.camelToKebab;
exports.getKeyAliases = __webpack_exports__.getKeyAliases;
exports.isRecord = __webpack_exports__.isRecord;
exports.kebabToCamel = __webpack_exports__.kebabToCamel;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "camelToKebab",
    "getKeyAliases",
    "isRecord",
    "kebabToCamel"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});
