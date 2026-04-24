import { Page } from "./base-page.mjs";
class PuppeteerWebPage extends Page {
    constructor(page, opts){
        super(page, 'puppeteer', opts);
    }
}
export { PuppeteerWebPage };

//# sourceMappingURL=page.mjs.map