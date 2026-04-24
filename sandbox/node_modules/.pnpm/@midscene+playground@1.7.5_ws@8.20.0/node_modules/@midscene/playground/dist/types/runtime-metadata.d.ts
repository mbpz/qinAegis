import { type PlaygroundPreviewDescriptor } from './platform';
export interface PlaygroundRuntimeInfo {
    /** Stable platform key, e.g. `android`, `ios`, `web`, `computer`. */
    platformId?: string;
    /** User-facing runtime title, e.g. `Midscene Android Playground`. */
    title?: string;
    /** Human-readable platform summary, e.g. `Android playground platform descriptor`. */
    platformDescription?: string;
    interface: {
        type: string;
        description?: string;
    };
    preview: PlaygroundPreviewDescriptor;
    executionUxHints: string[];
    metadata: Record<string, unknown>;
}
interface BuildRuntimeInfoInput {
    platformId?: string;
    title?: string;
    platformDescription?: string;
    interfaceType?: string;
    interfaceDescription?: string;
    preview?: PlaygroundPreviewDescriptor;
    metadata?: Record<string, unknown>;
    supportsScreenshot?: boolean;
    mjpegStreamUrl?: string;
    scrcpyPort?: number;
}
export declare function normalizeExecutionUxHints(metadata?: Record<string, unknown>): string[];
export declare function resolvePreviewDescriptor(input: Omit<BuildRuntimeInfoInput, 'platformId' | 'title' | 'platformDescription' | 'interfaceType' | 'interfaceDescription' | 'metadata'>): PlaygroundPreviewDescriptor;
export declare function buildRuntimeInfo(input: BuildRuntimeInfoInput): PlaygroundRuntimeInfo;
export {};
