function definePlaygroundPlatform(descriptor) {
    return descriptor;
}
function createScreenshotPreviewDescriptor(overrides = {}) {
    return {
        kind: 'screenshot',
        screenshotPath: '/screenshot',
        capabilities: [
            {
                kind: 'screenshot',
                label: 'Screenshot polling',
                live: false
            }
        ],
        ...overrides
    };
}
function createMjpegPreviewDescriptor(overrides = {}) {
    return {
        kind: 'mjpeg',
        screenshotPath: '/screenshot',
        mjpegPath: '/mjpeg',
        capabilities: [
            {
                kind: 'mjpeg',
                label: 'MJPEG streaming',
                live: true
            },
            {
                kind: 'screenshot',
                label: 'Screenshot fallback',
                live: false
            }
        ],
        ...overrides
    };
}
function createScrcpyPreviewDescriptor(custom = {}, overrides = {}) {
    return {
        kind: 'scrcpy',
        screenshotPath: '/screenshot',
        capabilities: [
            {
                kind: 'scrcpy',
                label: 'scrcpy streaming',
                live: true
            },
            {
                kind: 'screenshot',
                label: 'Screenshot fallback',
                live: false
            }
        ],
        custom,
        ...overrides
    };
}
function resolvePreparedLaunchOptions(prepared, overrides = {}) {
    return {
        ...prepared.launchOptions || {},
        ...overrides
    };
}
export { createMjpegPreviewDescriptor, createScrcpyPreviewDescriptor, createScreenshotPreviewDescriptor, definePlaygroundPlatform, resolvePreparedLaunchOptions };

//# sourceMappingURL=platform.mjs.map