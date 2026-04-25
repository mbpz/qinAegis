import type { Agent } from '@midscene/core/agent';
import { type CorsOptions } from 'cors';
import PlaygroundServer from './server';
import type { AgentFactory } from './types';
export interface LaunchPlaygroundOptions {
    /**
     * Port to start the playground server on
     * @default 5800
     */
    port?: number;
    /**
     * Whether to automatically open the playground in browser
     * @default true
     */
    openBrowser?: boolean;
    /**
     * Custom browser command to open playground
     * @default 'open' on macOS, 'start' on Windows, 'xdg-open' on Linux
     */
    browserCommand?: string;
    /**
     * Whether to show server logs
     * @default true
     */
    verbose?: boolean;
    /**
     * Fixed ID for the playground server instance
     * If provided, the same ID will be used across restarts,
     * allowing chat history to persist
     * @default undefined (generates random UUID)
     */
    id?: string;
    /**
     * Whether to enable CORS (Cross-Origin Resource Sharing)
     * @default false
     */
    enableCors?: boolean;
    /**
     * Custom static assets directory for the playground frontend
     * @default bundled static assets from @midscene/playground
     */
    staticPath?: string;
    /**
     * Hook for configuring the PlaygroundServer before launch
     * Useful for adding custom middleware beyond the built-in CORS option
     */
    configureServer?: (server: PlaygroundServer) => void | Promise<void>;
    /**
     * CORS configuration options
     * @default only allows loopback browser origins when enableCors is true
     */
    corsOptions?: CorsOptions;
}
export interface LaunchPlaygroundResult {
    /**
     * The playground server instance
     */
    server: PlaygroundServer;
    /**
     * The server port
     */
    port: number;
    /**
     * The server host
     */
    host: string;
    /**
     * Function to gracefully shutdown the playground
     */
    close: () => Promise<void>;
}
/**
 * Create a playground launcher from an already initialized agent instance
 */
export declare function playgroundForAgent(agent: Agent): {
    /**
     * Launch the playground server with optional configuration
     */
    launch(options?: LaunchPlaygroundOptions): Promise<LaunchPlaygroundResult>;
};
/**
 * Create a playground launcher from an agent factory
 * Useful for device-backed agents that need to be recreated after cancellation
 */
export declare function playgroundForAgentFactory(agentFactory: AgentFactory): {
    /**
     * Launch the playground server with optional configuration
     */
    launch(options?: LaunchPlaygroundOptions): Promise<LaunchPlaygroundResult>;
};
export declare function playgroundForSessionManager(): {
    /**
     * Launch the playground server with optional configuration
     */
    launch(options?: LaunchPlaygroundOptions): Promise<LaunchPlaygroundResult>;
};
