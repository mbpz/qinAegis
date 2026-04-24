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
    maskConfig: ()=>maskConfig,
    parseJson: ()=>parseJson
});
const maskKey = (key, maskChar = '*')=>{
    if ('string' != typeof key || 0 === key.length) return key;
    const prefixLen = 3;
    const suffixLen = 3;
    const keepLength = prefixLen + suffixLen;
    if (key.length <= keepLength) return key;
    const prefix = key.substring(0, prefixLen);
    const suffix = key.substring(key.length - suffixLen);
    const maskLength = key.length - keepLength;
    const mask = maskChar.repeat(maskLength);
    return `${prefix}${mask}${suffix}`;
};
const maskConfig = (config)=>Object.fromEntries(Object.entries(config).map(([key, value])=>{
        if (!value) return [
            key,
            value
        ];
        if ('string' == typeof value && /key/i.test(key)) return [
            key,
            maskKey(value)
        ];
        if ('object' == typeof value) {
            const valueStr = JSON.stringify(value);
            if (/key/i.test(valueStr)) return [
                key,
                maskKey(valueStr)
            ];
        }
        return [
            key,
            value
        ];
    }));
const parseJson = (key, value)=>{
    if (value) try {
        return JSON.parse(value);
    } catch (e) {
        throw new Error(`Failed to parse ${key} as a JSON. ${e.message}`, {
            cause: e
        });
    }
};
exports.maskConfig = __webpack_exports__.maskConfig;
exports.parseJson = __webpack_exports__.parseJson;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "maskConfig",
    "parseJson"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});
