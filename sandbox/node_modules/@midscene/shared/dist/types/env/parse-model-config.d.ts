import { DEFAULT_MODEL_CONFIG_KEYS, type DEFAULT_MODEL_CONFIG_KEYS_LEGACY, INSIGHT_MODEL_CONFIG_KEYS, PLANNING_MODEL_CONFIG_KEYS } from './constants';
import { type IModelConfig, type TIntent, type TModelFamily, UITarsModelVersion } from './types';
type TModelConfigKeys = typeof INSIGHT_MODEL_CONFIG_KEYS | typeof PLANNING_MODEL_CONFIG_KEYS | typeof DEFAULT_MODEL_CONFIG_KEYS | typeof DEFAULT_MODEL_CONFIG_KEYS_LEGACY;
/**
 * Get UI-TARS model version from model family
 * @param modelFamily - The model family value
 * @returns UITarsModelVersion if the model family is a UI-TARS variant, undefined otherwise
 */
export declare const getUITarsModelVersion: (modelFamily?: TModelFamily) => UITarsModelVersion | undefined;
/**
 * Validate model family value
 * @param modelFamily - The model family value to validate
 * @throws Error if the model family is invalid
 */
export declare const validateModelFamily: (modelFamily?: TModelFamily) => void;
/**
 * Convert legacy environment variables to model family
 * @param provider - Environment variable provider (e.g., process.env)
 * @returns The corresponding model family value, or undefined if no legacy config is found
 */
export declare const legacyConfigToModelFamily: (provider: Record<string, string | undefined>) => TModelFamily | undefined;
/**
 * Parse OpenAI SDK config
 */
export declare const parseOpenaiSdkConfig: ({ keys, provider, useLegacyLogic, }: {
    keys: TModelConfigKeys;
    provider: Record<string, string | undefined>;
    useLegacyLogic?: boolean;
}) => IModelConfig;
export declare const decideModelConfigFromIntentConfig: (intent: TIntent, configMap: Record<string, string | undefined>) => IModelConfig | undefined;
export {};
