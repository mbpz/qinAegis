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
    ScreenshotItem: ()=>ScreenshotItem
});
const external_node_fs_namespaceObject = require("node:fs");
const utils_namespaceObject = require("@midscene/shared/utils");
const html_utils_js_namespaceObject = require("./dump/html-utils.js");
const screenshot_store_js_namespaceObject = require("./dump/screenshot-store.js");
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
function detectFormat(base64) {
    if (base64.startsWith('data:image/jpeg')) return 'jpeg';
    if (base64.startsWith('data:image/jpg')) return 'jpeg';
    return 'png';
}
class ScreenshotItem {
    static create(base64, capturedAt) {
        return new ScreenshotItem((0, utils_namespaceObject.uuid)(), base64, capturedAt);
    }
    get id() {
        return this._id;
    }
    get format() {
        return this._format;
    }
    get extension() {
        return 'jpeg' === this._format ? 'jpeg' : 'png';
    }
    get capturedAt() {
        return this._capturedAt;
    }
    get base64() {
        if (null !== this._base64) return this._base64;
        const loadFromFile = ()=>{
            if (null === this._persistedPath) throw new Error(`Screenshot ${this._id}: file recovery path missing`);
            const buffer = (0, external_node_fs_namespaceObject.readFileSync)(this._persistedPath);
            return `data:image/${this._format};base64,${buffer.toString('base64')}`;
        };
        const loadFromInline = ()=>{
            if (null === this._persistedHtmlPath) throw new Error(`Screenshot ${this._id}: HTML recovery path missing`);
            const data = (0, html_utils_js_namespaceObject.extractImageByIdSync)(this._persistedHtmlPath, this._id);
            if (data) return data;
            throw new Error(`Screenshot ${this._id}: cannot recover from HTML (id not found in ${this._persistedHtmlPath})`);
        };
        if (this._serializedRef?.storage === 'file') return loadFromFile();
        if (this._serializedRef?.storage === 'inline') return loadFromInline();
        if (null !== this._persistedPath) return loadFromFile();
        if (null !== this._persistedHtmlPath) return loadFromInline();
        throw new Error(`Screenshot ${this._id}: base64 data released without recovery path`);
    }
    hasBase64() {
        return null !== this._base64;
    }
    markPersistedInline(htmlPath) {
        const ref = this.createRef('inline');
        this._serializedRef = ref;
        this._persistedHtmlPath = htmlPath;
        this._base64 = null;
        return ref;
    }
    registerPersistedFileCopy(relativePath, absolutePath) {
        const ref = this.createRef('file', relativePath);
        this._persistedPath = absolutePath;
        this._base64 = null;
        return ref;
    }
    markPersistedToPath(relativePath, absolutePath) {
        const ref = this.registerPersistedFileCopy(relativePath, absolutePath);
        this._serializedRef = ref;
        return ref;
    }
    toSerializable() {
        return this._serializedRef ?? {
            type: 'midscene_screenshot_ref',
            id: this._id,
            capturedAt: this._capturedAt,
            mimeType: 'jpeg' === this._format ? 'image/jpeg' : 'image/png',
            storage: 'inline'
        };
    }
    static isSerialized(value) {
        return null !== (0, screenshot_store_js_namespaceObject.normalizeScreenshotRef)(value);
    }
    createRef(storage, relativePath) {
        const baseRef = {
            type: 'midscene_screenshot_ref',
            id: this._id,
            capturedAt: this._capturedAt,
            mimeType: 'jpeg' === this._format ? 'image/jpeg' : 'image/png',
            storage
        };
        if ('file' === storage) return {
            ...baseRef,
            storage,
            path: relativePath
        };
        return baseRef;
    }
    get rawBase64() {
        return this.base64.replace(/^data:image\/(png|jpeg|jpg);base64,/, '');
    }
    constructor(id, base64, capturedAt){
        _define_property(this, "_id", void 0);
        _define_property(this, "_base64", void 0);
        _define_property(this, "_format", void 0);
        _define_property(this, "_capturedAt", void 0);
        _define_property(this, "_serializedRef", null);
        _define_property(this, "_persistedPath", null);
        _define_property(this, "_persistedHtmlPath", null);
        this._id = id;
        this._base64 = base64;
        this._format = detectFormat(base64);
        this._capturedAt = capturedAt;
    }
}
exports.ScreenshotItem = __webpack_exports__.ScreenshotItem;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "ScreenshotItem"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=screenshot-item.js.map