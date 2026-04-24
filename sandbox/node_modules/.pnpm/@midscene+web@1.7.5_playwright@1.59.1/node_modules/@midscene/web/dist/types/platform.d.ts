import type { Agent } from '@midscene/core/agent';
import { type AgentFactory, type LaunchPlaygroundOptions, type PlaygroundPreviewDescriptor } from '@midscene/playground';
export interface WebPlatformOptions {
    agent?: Agent;
    agentFactory?: AgentFactory;
    title?: string;
    preview?: PlaygroundPreviewDescriptor;
    launchOptions?: LaunchPlaygroundOptions;
}
export declare const webPlaygroundPlatform: import("@midscene/playground").PlaygroundPlatformDescriptor<WebPlatformOptions | undefined>;
