import { createMjpegPreviewDescriptor, createScrcpyPreviewDescriptor, createScreenshotPreviewDescriptor } from "./platform.mjs";
function normalizeExecutionUxHints(metadata) {
    if (!metadata) return [];
    const fromHints = metadata.executionUxHints;
    if (Array.isArray(fromHints)) return fromHints.filter((value)=>'string' == typeof value && value.length > 0);
    const fromSingle = metadata.executionUx;
    if ('string' == typeof fromSingle && fromSingle.length > 0) return [
        fromSingle
    ];
    return [];
}
function resolvePreviewDescriptor(input) {
    if (input.preview) return input.preview;
    if ('number' == typeof input.scrcpyPort) return createScrcpyPreviewDescriptor({
        scrcpyPort: input.scrcpyPort
    });
    if (input.mjpegStreamUrl) return createMjpegPreviewDescriptor();
    if (input.supportsScreenshot) return createScreenshotPreviewDescriptor();
    return {
        kind: 'none',
        capabilities: []
    };
}
function buildRuntimeInfo(input) {
    const interfaceType = input.interfaceType || 'Unknown';
    return {
        platformId: input.platformId,
        title: input.title,
        platformDescription: input.platformDescription,
        interface: {
            type: interfaceType,
            description: input.interfaceDescription
        },
        preview: resolvePreviewDescriptor(input),
        executionUxHints: normalizeExecutionUxHints(input.metadata),
        metadata: {
            ...input.metadata || {}
        }
    };
}
export { buildRuntimeInfo, normalizeExecutionUxHints, resolvePreviewDescriptor };

//# sourceMappingURL=runtime-metadata.mjs.map