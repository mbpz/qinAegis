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
    createNamespacedInitArgSchema: ()=>createNamespacedInitArgSchema,
    extractNamespacedArgs: ()=>extractNamespacedArgs,
    sanitizeNamespacedArgs: ()=>sanitizeNamespacedArgs
});
const external_key_alias_utils_js_namespaceObject = require("../key-alias-utils.js");
function readAliasedValue(args, key) {
    for (const alias of (0, external_key_alias_utils_js_namespaceObject.getKeyAliases)(key))if (alias in args) return args[alias];
}
function readNamespacedArg(args, namespace, key) {
    const namespacedArgs = readAliasedValue(args, namespace);
    if ((0, external_key_alias_utils_js_namespaceObject.isRecord)(namespacedArgs)) {
        const nestedValue = readAliasedValue(namespacedArgs, key);
        if (void 0 !== nestedValue) return nestedValue;
    }
    const dottedValue = readAliasedValue(args, `${namespace}.${key}`);
    if (void 0 !== dottedValue) return dottedValue;
    const directValue = readAliasedValue(args, key);
    if (void 0 !== directValue) return directValue;
}
function extractNamespacedArgs(args, namespace, keys) {
    const extracted = {};
    for (const key of keys){
        const value = readNamespacedArg(args, namespace, key);
        if (void 0 !== value) extracted[key] = value;
    }
    return Object.keys(extracted).length > 0 ? extracted : void 0;
}
function sanitizeNamespacedArgs(args, namespace, keys) {
    const excludedKeys = new Set((0, external_key_alias_utils_js_namespaceObject.getKeyAliases)(namespace));
    for (const key of keys){
        for (const alias of (0, external_key_alias_utils_js_namespaceObject.getKeyAliases)(key))excludedKeys.add(alias);
        for (const alias of (0, external_key_alias_utils_js_namespaceObject.getKeyAliases)(`${namespace}.${key}`))excludedKeys.add(alias);
    }
    return Object.fromEntries(Object.entries(args).filter(([key])=>!excludedKeys.has(key)));
}
function createNamespacedInitArgSchema(namespace, shape) {
    return Object.fromEntries(Object.entries(shape).map(([key, value])=>[
            `${namespace}.${key}`,
            value
        ]));
}
exports.createNamespacedInitArgSchema = __webpack_exports__.createNamespacedInitArgSchema;
exports.extractNamespacedArgs = __webpack_exports__.extractNamespacedArgs;
exports.sanitizeNamespacedArgs = __webpack_exports__.sanitizeNamespacedArgs;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "createNamespacedInitArgSchema",
    "extractNamespacedArgs",
    "sanitizeNamespacedArgs"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});
