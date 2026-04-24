/**
 * Canvas-based fallback for image processing when Photon WASM fails to load.
 * Provides a compatible API with Photon for browser environments.
 */
/**
 * Canvas-based image class that mimics PhotonImage API
 */
export declare class CanvasImage {
    private canvas;
    private ctx;
    private _width;
    private _height;
    constructor(canvas: HTMLCanvasElement);
    get_width(): number;
    get_height(): number;
    get_raw_pixels(): Uint8Array;
    get_bytes_jpeg(quality: number): Uint8Array;
    free(): void;
    _getCanvas(): HTMLCanvasElement;
    _getContext(): CanvasRenderingContext2D;
    /**
     * Create a CanvasImage from a base64 string
     */
    static new_from_base64(base64Body: string): Promise<CanvasImage>;
    /**
     * Create a CanvasImage from a byte array (async version)
     */
    static new_from_byteslice(bytes: Uint8Array): Promise<CanvasImage>;
}
/**
 * Sampling filter enum (compatible with Photon)
 */
export declare const CanvasSamplingFilter: {
    readonly Nearest: "nearest";
    readonly Triangle: "triangle";
    readonly CatmullRom: "catmullrom";
    readonly Gaussian: "gaussian";
    readonly Lanczos3: "lanczos3";
};
/**
 * RGBA color class (compatible with Photon)
 */
export declare class CanvasRgba {
    r: number;
    g: number;
    b: number;
    a: number;
    constructor(r: number, g: number, b: number, a: number);
}
/**
 * Resize an image
 */
export declare function canvasResize(image: CanvasImage, newWidth: number, newHeight: number, _filter: string): CanvasImage;
/**
 * Crop an image
 */
export declare function canvasCrop(image: CanvasImage, x1: number, y1: number, x2: number, y2: number): CanvasImage;
/**
 * Add padding to the right of an image
 */
export declare function canvasPaddingRight(image: CanvasImage, padding: number, color: CanvasRgba): CanvasImage;
/**
 * Add padding to the bottom of an image
 */
export declare function canvasPaddingBottom(image: CanvasImage, padding: number, color: CanvasRgba): CanvasImage;
/**
 * Add uniform padding to an image
 */
export declare function canvasPaddingUniform(image: CanvasImage, padding: number, color: CanvasRgba): CanvasImage;
/**
 * Add padding to the left of an image
 */
export declare function canvasPaddingLeft(image: CanvasImage, padding: number, color: CanvasRgba): CanvasImage;
/**
 * Add padding to the top of an image
 */
export declare function canvasPaddingTop(image: CanvasImage, padding: number, color: CanvasRgba): CanvasImage;
/**
 * Watermark an image (overlay one image on another)
 */
export declare function canvasWatermark(base: CanvasImage, overlay: CanvasImage, x: number, y: number): CanvasImage;
/**
 * Create and return the canvas fallback module with Photon-compatible API
 */
export declare function createCanvasFallbackModule(): {
    PhotonImage: typeof CanvasImage;
    SamplingFilter: {
        readonly Nearest: "nearest";
        readonly Triangle: "triangle";
        readonly CatmullRom: "catmullrom";
        readonly Gaussian: "gaussian";
        readonly Lanczos3: "lanczos3";
    };
    resize: typeof canvasResize;
    crop: typeof canvasCrop;
    open_image: () => never;
    base64_to_image: typeof CanvasImage.new_from_base64;
    padding_uniform: typeof canvasPaddingUniform;
    padding_left: typeof canvasPaddingLeft;
    padding_right: typeof canvasPaddingRight;
    padding_top: typeof canvasPaddingTop;
    padding_bottom: typeof canvasPaddingBottom;
    watermark: typeof canvasWatermark;
    Rgba: typeof CanvasRgba;
};
