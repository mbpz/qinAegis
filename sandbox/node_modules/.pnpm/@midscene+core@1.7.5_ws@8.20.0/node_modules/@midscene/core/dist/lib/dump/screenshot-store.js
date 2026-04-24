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
    normalizeScreenshotRef: ()=>normalizeScreenshotRef,
    ScreenshotStore: ()=>ScreenshotStore,
    resolveScreenshotSource: ()=>resolveScreenshotSource
});
const external_node_fs_namespaceObject = require("node:fs");
const promises_namespaceObject = require("node:fs/promises");
const external_node_path_namespaceObject = require("node:path");
const external_html_utils_js_namespaceObject = require("./html-utils.js");
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
function normalizeScreenshotRef(value) {
    if ('object' != typeof value || null === value) return null;
    const record = value;
    if ('midscene_screenshot_ref' === record.type && 'string' == typeof record.id && 'number' == typeof record.capturedAt && ('inline' === record.storage || 'file' === record.storage) && ('image/png' === record.mimeType || 'image/jpeg' === record.mimeType)) {
        if ('file' === record.storage && 'string' != typeof record.path) return null;
        return record;
    }
    return null;
}
function extensionByMimeType(mimeType) {
    return 'image/jpeg' === mimeType ? 'jpeg' : 'png';
}
function resolveScreenshotSource(refInput, options) {
    const ref = normalizeScreenshotRef(refInput);
    const id = ref?.id ?? options.fallbackId;
    const mimeType = ref?.mimeType ?? options.fallbackMimeType;
    if (!id || !mimeType) throw new Error('ScreenshotStore: screenshot id and mimeType are required to resolve screenshot');
    const resolveReportRelativePath = (filePath)=>(0, external_node_path_namespaceObject.isAbsolute)(filePath) ? filePath : (0, external_node_path_namespaceObject.join)((0, external_node_path_namespaceObject.dirname)(options.reportPath), filePath);
    if (ref?.storage === 'file') {
        if (!ref.path) throw new Error(`ScreenshotStore: screenshot ref "${ref.id}" missing file path`);
        const explicitFilePath = resolveReportRelativePath(ref.path);
        if ((0, external_node_fs_namespaceObject.existsSync)(explicitFilePath)) return {
            type: 'file',
            id,
            mimeType,
            filePath: explicitFilePath
        };
    }
    const inlineDataUri = (0, external_html_utils_js_namespaceObject.extractImageByIdSync)(options.reportPath, id);
    if (inlineDataUri) return {
        type: 'data-uri',
        id,
        mimeType,
        dataUri: inlineDataUri
    };
    const siblingScreenshotPath = (0, external_node_path_namespaceObject.join)((0, external_node_path_namespaceObject.dirname)(options.reportPath), 'screenshots', `${id}.${extensionByMimeType(mimeType)}`);
    if ((0, external_node_fs_namespaceObject.existsSync)(siblingScreenshotPath)) return {
        type: 'file',
        id,
        mimeType,
        filePath: siblingScreenshotPath
    };
    throw new Error(`ScreenshotStore: cannot resolve screenshot "${id}" from ${options.reportPath}`);
}
class ScreenshotStore {
    async persist(screenshot) {
        const shouldWriteFileCopy = 'directory' === this.mode || this.alsoWriteFileCopy;
        const fileRef = shouldWriteFileCopy ? await this.persistToSharedFileIfNeeded(screenshot, {
            markAsPersisted: 'directory' === this.mode
        }) : null;
        if ('inline' === this.mode) {
            if (!this.writeInlineImage) throw new Error('ScreenshotStore: writeInlineImage is required in inline mode');
            if (!this.writtenInlineIds.has(screenshot.id)) {
                await this.writeInlineImage(screenshot.id, screenshot.base64);
                this.writtenInlineIds.add(screenshot.id);
            }
            return screenshot.markPersistedInline(this.reportPath);
        }
        if (!fileRef) throw new Error('ScreenshotStore: file persistence is required in directory mode');
        return fileRef;
    }
    async persistToSharedFileIfNeeded(screenshot, options) {
        const screenshotsDir = this.screenshotsDir;
        if (!screenshotsDir) throw new Error('ScreenshotStore: screenshotsDir is required when file persistence is enabled');
        if (!(0, external_node_fs_namespaceObject.existsSync)(screenshotsDir)) (0, external_node_fs_namespaceObject.mkdirSync)(screenshotsDir, {
            recursive: true
        });
        const relativePath = `./screenshots/${screenshot.id}.${screenshot.extension}`;
        const absolutePath = (0, external_node_path_namespaceObject.join)(screenshotsDir, `${screenshot.id}.${screenshot.extension}`);
        if (!this.writtenFileIds.has(screenshot.id)) {
            const buffer = Buffer.from(screenshot.rawBase64, 'base64');
            await (0, promises_namespaceObject.writeFile)(absolutePath, buffer);
            this.writtenFileIds.add(screenshot.id);
        }
        if (options.markAsPersisted) return screenshot.markPersistedToPath(relativePath, absolutePath);
        return screenshot.registerPersistedFileCopy(relativePath, absolutePath);
    }
    loadBase64(refInput) {
        const ref = normalizeScreenshotRef(refInput);
        if (!ref) throw new Error('ScreenshotStore: invalid screenshot reference');
        const resolved = resolveScreenshotSource(ref, {
            reportPath: this.reportPath
        });
        if ('data-uri' === resolved.type) return resolved.dataUri;
        const data = (0, external_node_fs_namespaceObject.readFileSync)(resolved.filePath);
        return `data:${resolved.mimeType};base64,${data.toString('base64')}`;
    }
    cleanup() {
        if ('directory' === this.mode && this.screenshotsDir && (0, external_node_fs_namespaceObject.existsSync)(this.screenshotsDir)) (0, external_node_fs_namespaceObject.rmSync)(this.screenshotsDir, {
            recursive: true,
            force: true
        });
        this.writtenInlineIds.clear();
        this.writtenFileIds.clear();
    }
    constructor(options){
        _define_property(this, "mode", void 0);
        _define_property(this, "reportPath", void 0);
        _define_property(this, "screenshotsDir", void 0);
        _define_property(this, "writeInlineImage", void 0);
        _define_property(this, "alsoWriteFileCopy", void 0);
        _define_property(this, "writtenInlineIds", new Set());
        _define_property(this, "writtenFileIds", new Set());
        this.mode = options.mode;
        this.reportPath = options.reportPath;
        this.screenshotsDir = options.screenshotsDir;
        this.writeInlineImage = options.writeInlineImage;
        this.alsoWriteFileCopy = options.alsoWriteFileCopy ?? options.ensureFileCopy ?? false;
    }
}
exports.ScreenshotStore = __webpack_exports__.ScreenshotStore;
exports.normalizeScreenshotRef = __webpack_exports__.normalizeScreenshotRef;
exports.resolveScreenshotSource = __webpack_exports__.resolveScreenshotSource;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "ScreenshotStore",
    "normalizeScreenshotRef",
    "resolveScreenshotSource"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=screenshot-store.js.map