import type { Page as PlaywrightPageType } from 'playwright';
import { Page as BasePage } from '../puppeteer/base-page';
import type { WebPageOpt } from '../web-element';
export declare class WebPage extends BasePage<'playwright', PlaywrightPageType> {
    private playwrightFileChooserHandler?;
    constructor(page: PlaywrightPageType, opts?: WebPageOpt);
    registerFileChooserListener(handler: (chooser: import('@midscene/core/device').FileChooserHandler) => Promise<void>): Promise<{
        dispose: () => void;
        getError: () => Error | undefined;
    }>;
}
