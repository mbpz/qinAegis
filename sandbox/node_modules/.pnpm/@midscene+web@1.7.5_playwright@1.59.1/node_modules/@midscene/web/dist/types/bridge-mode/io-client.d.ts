export declare class BridgeClient {
    endpoint: string;
    onBridgeCall: (method: string, args: any[]) => Promise<any>;
    onDisconnect?: (() => void) | undefined;
    private socket;
    serverVersion: string | null;
    constructor(endpoint: string, onBridgeCall: (method: string, args: any[]) => Promise<any>, onDisconnect?: (() => void) | undefined);
    connect(): Promise<unknown>;
    disconnect(): void;
}
