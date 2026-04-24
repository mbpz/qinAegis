import type { GlobalConfigManager } from './global-config-manager';
import type { CreateOpenAIClientFn, IModelConfig, TIntent, TModelConfig } from './types';
export declare class ModelConfigManager {
    private modelConfigMap;
    private isInitialized;
    private isolatedMode;
    private globalConfigManager;
    private modelConfig?;
    private createOpenAIClientFn?;
    constructor(modelConfig?: TModelConfig, createOpenAIClientFn?: CreateOpenAIClientFn);
    private initialize;
    private normalizeModelConfig;
    /**
     * should only be called by GlobalConfigManager
     */
    clearModelConfigMap(): void;
    /**
     * if isolatedMode is true, modelConfigMap was initialized in constructor and can't be changed
     * if isolatedMode is false, modelConfigMap can be changed by process.env so we need to recalculate it when it's undefined
     */
    getModelConfig(intent: TIntent): IModelConfig;
    getUploadTestServerUrl(): string | undefined;
    registerGlobalConfigManager(globalConfigManager: GlobalConfigManager): void;
    throwErrorIfNonVLModel(): void;
}
