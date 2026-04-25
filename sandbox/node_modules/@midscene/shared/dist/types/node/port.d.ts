/**
 * Check if a port is available
 */
export declare function isPortAvailable(port: number): Promise<boolean>;
/**
 * Find an available port starting from the given port
 */
export declare function findAvailablePort(startPort: number, maxAttempts?: number): Promise<number>;
