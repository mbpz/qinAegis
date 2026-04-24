import { type LaunchPlaygroundOptions, type LaunchPlaygroundResult } from './launcher';
import { type PreparedPlaygroundPlatform } from './platform';
export declare function launchPreparedPlaygroundPlatform(prepared: PreparedPlaygroundPlatform, overrides?: LaunchPlaygroundOptions): Promise<LaunchPlaygroundResult>;
