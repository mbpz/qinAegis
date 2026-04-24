"use strict";
var __webpack_require__ = {};
(()=>{
    __webpack_require__.d = (exports1, definition)=>{
        for(var key in definition)if (__webpack_require__.o(definition, key) && !__webpack_require__.o(exports1, key)) Object.defineProperty(exports1, key, {
            enumerable: true,
            get: definition[key]
        });
    };
})();
(()=>{
    __webpack_require__.o = (obj, prop)=>Object.prototype.hasOwnProperty.call(obj, prop);
})();
(()=>{
    __webpack_require__.r = (exports1)=>{
        if ('undefined' != typeof Symbol && Symbol.toStringTag) Object.defineProperty(exports1, Symbol.toStringTag, {
            value: 'Module'
        });
        Object.defineProperty(exports1, '__esModule', {
            value: true
        });
    };
})();
var __webpack_exports__ = {};
__webpack_require__.r(__webpack_exports__);
__webpack_require__.d(__webpack_exports__, {
    WebPage: ()=>WebPage
});
const base_page_js_namespaceObject = require("../puppeteer/base-page.js");
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
class WebPage extends base_page_js_namespaceObject.Page {
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
exports.WebPage = __webpack_exports__.WebPage;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "WebPage"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=page.js.map