import { existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join } from "node:path";
import { extractImageByIdSync } from "./html-utils.mjs";
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
    const resolveReportRelativePath = (filePath)=>isAbsolute(filePath) ? filePath : join(dirname(options.reportPath), filePath);
    if (ref?.storage === 'file') {
        if (!ref.path) throw new Error(`ScreenshotStore: screenshot ref "${ref.id}" missing file path`);
        const explicitFilePath = resolveReportRelativePath(ref.path);
        if (existsSync(explicitFilePath)) return {
            type: 'file',
            id,
            mimeType,
            filePath: explicitFilePath
        };
    }
    const inlineDataUri = extractImageByIdSync(options.reportPath, id);
    if (inlineDataUri) return {
        type: 'data-uri',
        id,
        mimeType,
        dataUri: inlineDataUri
    };
    const siblingScreenshotPath = join(dirname(options.reportPath), 'screenshots', `${id}.${extensionByMimeType(mimeType)}`);
    if (existsSync(siblingScreenshotPath)) return {
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
        if (!existsSync(screenshotsDir)) mkdirSync(screenshotsDir, {
            recursive: true
        });
        const relativePath = `./screenshots/${screenshot.id}.${screenshot.extension}`;
        const absolutePath = join(screenshotsDir, `${screenshot.id}.${screenshot.extension}`);
        if (!this.writtenFileIds.has(screenshot.id)) {
            const buffer = Buffer.from(screenshot.rawBase64, 'base64');
            await writeFile(absolutePath, buffer);
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
        const data = readFileSync(resolved.filePath);
        return `data:${resolved.mimeType};base64,${data.toString('base64')}`;
    }
    cleanup() {
        if ('directory' === this.mode && this.screenshotsDir && existsSync(this.screenshotsDir)) rmSync(this.screenshotsDir, {
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
export { ScreenshotStore, normalizeScreenshotRef, resolveScreenshotSource };

//# sourceMappingURL=screenshot-store.mjs.map