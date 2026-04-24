export declare class CLIError extends Error {
    exitCode: number;
    constructor(message: string, exitCode?: number);
}
export declare function reportCLIError(error: unknown, log?: (message?: unknown, ...optionalParams: unknown[]) => void): number;
