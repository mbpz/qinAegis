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
    GlobalConfigManager: ()=>GlobalConfigManager
});
const external_init_debug_js_namespaceObject = require("./init-debug.js");
const external_types_js_namespaceObject = require("./types.js");
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
class GlobalConfigManager {
    getAllEnvConfig() {
        const envConfig = external_types_js_namespaceObject.ALL_ENV_KEYS.reduce((p, name)=>{
            p[name] = process.env[name];
            return p;
        }, Object.create(null));
        if (!this.override) return envConfig;
        {
            const { newConfig, extendMode } = this.override;
            if (extendMode) return {
                ...envConfig,
                ...newConfig
            };
            return {
                ...newConfig
            };
        }
    }
    getEnvConfigValue(key) {
        const allConfig = this.getAllEnvConfig();
        if (key === external_types_js_namespaceObject.MATCH_BY_POSITION) throw new Error('MATCH_BY_POSITION is discarded, use MIDSCENE_MODEL_FAMILY instead');
        if (!external_types_js_namespaceObject.STRING_ENV_KEYS.includes(key)) throw new Error(`getEnvConfigValue with key ${key} is not supported.`);
        const value = allConfig[key];
        this.keysHaveBeenRead[key] = true;
        if ('string' == typeof value) return value.trim();
        return value;
    }
    getEnvConfigInBoolean(key) {
        const allConfig = this.getAllEnvConfig();
        if (!external_types_js_namespaceObject.BOOLEAN_ENV_KEYS.includes(key)) throw new Error(`getEnvConfigInBoolean with key ${key} is not supported`);
        const value = allConfig[key];
        this.keysHaveBeenRead[key] = true;
        if (!value) return false;
        if (/^(true|1)$/i.test(value)) return true;
        if (/^(false|0)$/i.test(value)) return false;
        return !!value.trim();
    }
    getEnvConfigValueAsNumber(key) {
        if (!external_types_js_namespaceObject.STRING_ENV_KEYS.includes(key) && !external_types_js_namespaceObject.NUMBER_ENV_KEYS.includes(key)) throw new Error(`getEnvConfigValueAsNumber with key ${key} is not supported.`);
        const allConfig = this.getAllEnvConfig();
        const value = allConfig[key];
        this.keysHaveBeenRead[key] = true;
        if ('string' != typeof value) return;
        const trimmed = value.trim();
        if (!trimmed) return;
        const numValue = Number(trimmed);
        return Number.isNaN(numValue) ? void 0 : numValue;
    }
    registerModelConfigManager(globalModelConfigManager) {
        this.globalModelConfigManager = globalModelConfigManager;
    }
    overrideAIConfig(newConfig, extendMode = false) {
        for(const key in newConfig){
            if (![
                ...external_types_js_namespaceObject.GLOBAL_ENV_KEYS,
                ...external_types_js_namespaceObject.MODEL_ENV_KEYS
            ].includes(key)) throw new Error(`Failed to override AI config, invalid key: ${key}`);
            const value = newConfig[key];
            if ('string' != typeof value) throw new Error(`Failed to override AI config, value for key ${key} must be a string, but got with type ${typeof value}`);
            if (this.keysHaveBeenRead[key]) console.warn(`Warning: try to override AI config with key ${key} ,but it has been read.`);
        }
        const savedNewConfig = extendMode ? {
            ...this.override?.newConfig,
            ...newConfig
        } : newConfig;
        this.override = {
            newConfig: {
                ...savedNewConfig
            },
            extendMode
        };
        if (!this.globalModelConfigManager) throw new Error('globalModelConfigManager is not registered, which should not happen');
        this.globalModelConfigManager.clearModelConfigMap();
    }
    constructor(){
        _define_property(this, "override", void 0);
        _define_property(this, "keysHaveBeenRead", {});
        _define_property(this, "globalModelConfigManager", void 0);
        (0, external_init_debug_js_namespaceObject.initDebugConfig)();
    }
}
exports.GlobalConfigManager = __webpack_exports__.GlobalConfigManager;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "GlobalConfigManager"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});
