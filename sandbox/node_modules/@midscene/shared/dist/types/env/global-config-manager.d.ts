import type { ModelConfigManager } from './model-config-manager';
import { BOOLEAN_ENV_KEYS, GLOBAL_ENV_KEYS, NUMBER_ENV_KEYS, STRING_ENV_KEYS } from './types';
import { MODEL_ENV_KEYS } from './types';
/**
 * Collect global configs from process.env, overrideAIConfig, etc.
 * And provider methods to get merged config value
 */
export declare class GlobalConfigManager {
    private override;
    private keysHaveBeenRead;
    private globalModelConfigManager;
    constructor();
    /**
     * recalculate allEnvConfig every time because process.env can be updated any time
     */
    getAllEnvConfig(): Record<string, string | undefined>;
    getEnvConfigValue(key: (typeof STRING_ENV_KEYS)[number]): string | undefined;
    /**
     * read boolean only from process.env
     */
    getEnvConfigInBoolean(key: (typeof BOOLEAN_ENV_KEYS)[number]): boolean;
    /**
     * Read environment variable value and convert it to number.
     * Returns undefined if the value is not set or cannot be converted to a valid number.
     */
    getEnvConfigValueAsNumber(key: (typeof STRING_ENV_KEYS)[number] | (typeof NUMBER_ENV_KEYS)[number]): number | undefined;
    registerModelConfigManager(globalModelConfigManager: ModelConfigManager): void;
    /**
     * @deprecated use the modelConfig param in Agent constructor instead
     */
    overrideAIConfig(newConfig: Partial<Record<(typeof GLOBAL_ENV_KEYS)[number] | (typeof MODEL_ENV_KEYS)[number], string>>, extendMode?: boolean): void;
}
