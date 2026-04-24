import { getDebug } from "../logger.mjs";
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
const debug = getDebug('img:canvas-fallback');
class CanvasImage {
    get_width() {
        return this._width;
    }
    get_height() {
        return this._height;
    }
    get_raw_pixels() {
        const imageData = this.ctx.getImageData(0, 0, this._width, this._height);
        return new Uint8Array(imageData.data.buffer);
    }
    get_bytes_jpeg(quality) {
        const dataUrl = this.canvas.toDataURL('image/jpeg', quality / 100);
        const base64 = dataUrl.split(',')[1];
        const binary = atob(base64);
        const bytes = new Uint8Array(binary.length);
        for(let i = 0; i < binary.length; i++)bytes[i] = binary.charCodeAt(i);
        return bytes;
    }
    free() {}
    _getCanvas() {
        return this.canvas;
    }
    _getContext() {
        return this.ctx;
    }
    static async new_from_base64(base64Body) {
        return new Promise((resolve, reject)=>{
            const img = new Image();
            img.onload = ()=>{
                const canvas = document.createElement('canvas');
                canvas.width = img.width;
                canvas.height = img.height;
                const ctx = canvas.getContext('2d');
                if (!ctx) return void reject(new Error('Failed to get 2d context'));
                ctx.drawImage(img, 0, 0);
                resolve(new CanvasImage(canvas));
            };
            img.onerror = ()=>{
                reject(new Error('Failed to load image'));
            };
            if (base64Body.startsWith('data:')) img.src = base64Body;
            else img.src = `data:image/png;base64,${base64Body}`;
        });
    }
    static async new_from_byteslice(bytes) {
        return new Promise((resolve, reject)=>{
            const blob = new Blob([
                bytes
            ], {
                type: 'image/png'
            });
            const url = URL.createObjectURL(blob);
            const img = new Image();
            img.onload = ()=>{
                const canvas = document.createElement('canvas');
                canvas.width = img.width;
                canvas.height = img.height;
                const ctx = canvas.getContext('2d');
                if (!ctx) {
                    URL.revokeObjectURL(url);
                    reject(new Error('Failed to get 2d context'));
                    return;
                }
                ctx.drawImage(img, 0, 0);
                URL.revokeObjectURL(url);
                resolve(new CanvasImage(canvas));
            };
            img.onerror = ()=>{
                URL.revokeObjectURL(url);
                reject(new Error('Failed to load image from bytes'));
            };
            img.src = url;
        });
    }
    constructor(canvas){
        _define_property(this, "canvas", void 0);
        _define_property(this, "ctx", void 0);
        _define_property(this, "_width", void 0);
        _define_property(this, "_height", void 0);
        this.canvas = canvas;
        const ctx = canvas.getContext('2d');
        if (!ctx) throw new Error('Failed to get 2d context');
        this.ctx = ctx;
        this._width = canvas.width;
        this._height = canvas.height;
    }
}
const CanvasSamplingFilter = {
    Nearest: 'nearest',
    Triangle: 'triangle',
    CatmullRom: 'catmullrom',
    Gaussian: 'gaussian',
    Lanczos3: 'lanczos3'
};
class CanvasRgba {
    constructor(r, g, b, a){
        _define_property(this, "r", void 0);
        _define_property(this, "g", void 0);
        _define_property(this, "b", void 0);
        _define_property(this, "a", void 0);
        this.r = r;
        this.g = g;
        this.b = b;
        this.a = a;
    }
}
function canvasResize(image, newWidth, newHeight, _filter) {
    const canvas = document.createElement('canvas');
    canvas.width = newWidth;
    canvas.height = newHeight;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Failed to get 2d context');
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = 'high';
    ctx.drawImage(image._getCanvas(), 0, 0, newWidth, newHeight);
    return new CanvasImage(canvas);
}
function canvasCrop(image, x1, y1, x2, y2) {
    const width = x2 - x1;
    const height = y2 - y1;
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Failed to get 2d context');
    ctx.drawImage(image._getCanvas(), x1, y1, width, height, 0, 0, width, height);
    return new CanvasImage(canvas);
}
function canvasPaddingRight(image, padding, color) {
    const newWidth = image.get_width() + padding;
    const height = image.get_height();
    const canvas = document.createElement('canvas');
    canvas.width = newWidth;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Failed to get 2d context');
    ctx.fillStyle = `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a / 255})`;
    ctx.fillRect(0, 0, newWidth, height);
    ctx.drawImage(image._getCanvas(), 0, 0);
    return new CanvasImage(canvas);
}
function canvasPaddingBottom(image, padding, color) {
    const width = image.get_width();
    const newHeight = image.get_height() + padding;
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = newHeight;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Failed to get 2d context');
    ctx.fillStyle = `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a / 255})`;
    ctx.fillRect(0, 0, width, newHeight);
    ctx.drawImage(image._getCanvas(), 0, 0);
    return new CanvasImage(canvas);
}
function canvasPaddingUniform(image, padding, color) {
    const newWidth = image.get_width() + 2 * padding;
    const newHeight = image.get_height() + 2 * padding;
    const canvas = document.createElement('canvas');
    canvas.width = newWidth;
    canvas.height = newHeight;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Failed to get 2d context');
    ctx.fillStyle = `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a / 255})`;
    ctx.fillRect(0, 0, newWidth, newHeight);
    ctx.drawImage(image._getCanvas(), padding, padding);
    return new CanvasImage(canvas);
}
function canvasPaddingLeft(image, padding, color) {
    const newWidth = image.get_width() + padding;
    const height = image.get_height();
    const canvas = document.createElement('canvas');
    canvas.width = newWidth;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Failed to get 2d context');
    ctx.fillStyle = `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a / 255})`;
    ctx.fillRect(0, 0, newWidth, height);
    ctx.drawImage(image._getCanvas(), padding, 0);
    return new CanvasImage(canvas);
}
function canvasPaddingTop(image, padding, color) {
    const width = image.get_width();
    const newHeight = image.get_height() + padding;
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = newHeight;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Failed to get 2d context');
    ctx.fillStyle = `rgba(${color.r}, ${color.g}, ${color.b}, ${color.a / 255})`;
    ctx.fillRect(0, 0, width, newHeight);
    ctx.drawImage(image._getCanvas(), 0, padding);
    return new CanvasImage(canvas);
}
function canvasWatermark(base, overlay, x, y) {
    const canvas = document.createElement('canvas');
    canvas.width = base.get_width();
    canvas.height = base.get_height();
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Failed to get 2d context');
    ctx.drawImage(base._getCanvas(), 0, 0);
    ctx.drawImage(overlay._getCanvas(), x, y);
    return new CanvasImage(canvas);
}
function createCanvasFallbackModule() {
    debug('Creating Canvas fallback module');
    console.log('[midscene:img] Using Canvas fallback (Photon WASM not available)');
    return {
        PhotonImage: CanvasImage,
        SamplingFilter: CanvasSamplingFilter,
        resize: canvasResize,
        crop: canvasCrop,
        open_image: ()=>{
            throw new Error('open_image not supported in Canvas fallback');
        },
        base64_to_image: CanvasImage.new_from_base64,
        padding_uniform: canvasPaddingUniform,
        padding_left: canvasPaddingLeft,
        padding_right: canvasPaddingRight,
        padding_top: canvasPaddingTop,
        padding_bottom: canvasPaddingBottom,
        watermark: canvasWatermark,
        Rgba: CanvasRgba
    };
}
export { CanvasImage, CanvasRgba, CanvasSamplingFilter, canvasCrop, canvasPaddingBottom, canvasPaddingLeft, canvasPaddingRight, canvasPaddingTop, canvasPaddingUniform, canvasResize, canvasWatermark, createCanvasFallbackModule };
