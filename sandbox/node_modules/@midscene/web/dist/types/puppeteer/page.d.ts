import type { WebPageOpt } from '../web-element';
import type { Page as PuppeteerPageType } from 'puppeteer';
import { Page as BasePage } from './base-page';
export declare class PuppeteerWebPage extends BasePage<'puppeteer', PuppeteerPageType> {
    constructor(page: PuppeteerPageType, opts?: WebPageOpt);
}
