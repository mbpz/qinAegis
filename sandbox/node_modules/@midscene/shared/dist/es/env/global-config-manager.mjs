import { initDebugConfig } from "./init-debug.mjs";
import { ALL_ENV_KEYS, BOOLEAN_ENV_KEYS, GLOBAL_ENV_KEYS, MATCH_BY_POSITION, MODEL_ENV_KEYS, NUMBER_ENV_KEYS, STRING_ENV_KEYS } from "./types.mjs";
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
        const envConfig = ALL_ENV_KEYS.reduce((p, name)=>{
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
        if (key === MATCH_BY_POSITION) throw new Error('MATCH_BY_POSITION is discarded, use MIDSCENE_MODEL_FAMILY instead');
        if (!STRING_ENV_KEYS.includes(key)) throw new Error(`getEnvConfigValue with key ${key} is not supported.`);
        const value = allConfig[key];
        this.keysHaveBeenRead[key] = true;
        if ('string' == typeof value) return value.trim();
        return value;
    }
    getEnvConfigInBoolean(key) {
        const allConfig = this.getAllEnvConfig();
        if (!BOOLEAN_ENV_KEYS.includes(key)) throw new Error(`getEnvConfigInBoolean with key ${key} is not supported`);
        const value = allConfig[key];
        this.keysHaveBeenRead[key] = true;
        if (!value) return false;
        if (/^(true|1)$/i.test(value)) return true;
        if (/^(false|0)$/i.test(value)) return false;
        return !!value.trim();
    }
    getEnvConfigValueAsNumber(key) {
        if (!STRING_ENV_KEYS.includes(key) && !NUMBER_ENV_KEYS.includes(key)) throw new Error(`getEnvConfigValueAsNumber with key ${key} is not supported.`);
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
                ...GLOBAL_ENV_KEYS,
                ...MODEL_ENV_KEYS
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
        initDebugConfig();
    }
}
export { GlobalConfigManager };
