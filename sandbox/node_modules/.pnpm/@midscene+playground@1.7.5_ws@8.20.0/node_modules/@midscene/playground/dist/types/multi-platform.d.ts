import type { LaunchPlaygroundOptions, LaunchPlaygroundResult } from './launcher';
import type { PreparedPlaygroundPlatform } from './platform';
export interface RegisteredPlaygroundPlatform<TOptions = unknown> {
    id: string;
    label: string;
    description?: string;
    supportsStandalone?: boolean;
    unavailableReason?: string;
    metadata?: Record<string, unknown>;
    prepare: (options?: TOptions) => Promise<PreparedPlaygroundPlatform>;
    options?: TOptions;
}
export interface PrepareMultiPlatformPlaygroundOptions {
    platformId?: string;
    title?: string;
    description?: string;
    selectorFieldKey?: string;
    selectorVariant?: 'cards' | 'select';
    metadata?: Record<string, unknown>;
    launchOptions?: LaunchPlaygroundOptions;
}
export declare function prepareMultiPlatformPlayground(platforms: RegisteredPlaygroundPlatform[], options?: PrepareMultiPlatformPlaygroundOptions): Promise<PreparedPlaygroundPlatform>;
export declare function playgroundForPlatforms(platforms: RegisteredPlaygroundPlatform[], options?: PrepareMultiPlatformPlaygroundOptions): {
    launch(overrides?: LaunchPlaygroundOptions): Promise<LaunchPlaygroundResult>;
};
