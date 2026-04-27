import type { WebPageAgentOpt } from '../web-element';
import { Agent as PageAgent } from '@midscene/core/agent';
import type { Page as PuppeteerPage } from 'puppeteer';
import { PuppeteerWebPage } from './page';
export { PuppeteerWebPage } from './page';
export type { WebPageAgentOpt } from '../web-element';
export declare class PuppeteerAgent extends PageAgent<PuppeteerWebPage> {
    protected isRetryableContextError(error: unknown): boolean;
    constructor(page: PuppeteerPage, opts?: WebPageAgentOpt);
}
export { overrideAIConfig } from '@midscene/shared/env';
