import { Agent as PageAgent } from '@midscene/core/agent';
export declare class ChromeExtensionProxyPageAgent extends PageAgent {
    protected isRetryableContextError(error: unknown): boolean;
}
