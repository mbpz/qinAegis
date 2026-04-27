import { Page } from "../puppeteer/base-page.mjs";
function _define_property(obj, key, value) {
    if (key in obj) Object.defineProperty(obj, key, {
        value: value,
        enumerable: true,
        configurable: true,
        writable: true
    });
    else obj[key] = value;
    return obj;
}
class WebPage extends Page {
    async registerFileChooserListener(handler) {
        const page = this.underlyingPage;
        let capturedError;
        this.playwrightFileChooserHandler = async (chooser)=>{
            try {
                await handler({
                    accept: async (files)=>{
                        await chooser.setFiles(files);
                    }
                });
            } catch (error) {
                capturedError = error;
            }
        };
        page.on('filechooser', this.playwrightFileChooserHandler);
        return {
            dispose: ()=>{
                if (this.playwrightFileChooserHandler) {
                    page.off('filechooser', this.playwrightFileChooserHandler);
                    this.playwrightFileChooserHandler = void 0;
                }
            },
            getError: ()=>capturedError
        };
    }
    constructor(page, opts){
        super(page, 'playwright', opts), _define_property(this, "playwrightFileChooserHandler", void 0);
    }
}
export { WebPage };

//# sourceMappingURL=page.mjs.map