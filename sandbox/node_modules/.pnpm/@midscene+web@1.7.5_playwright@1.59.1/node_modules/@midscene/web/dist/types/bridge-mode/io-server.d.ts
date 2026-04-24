import { type BridgeCall } from './common';
export declare const killRunningServer: (port?: number, host?: string) => Promise<void>;
export declare class BridgeServer {
    host: string;
    port: number;
    onConnect?: (() => void) | undefined;
    onDisconnect?: ((reason: string) => void) | undefined;
    closeConflictServer?: boolean | undefined;
    private callId;
    private io;
    private socket;
    private listeningTimeoutId;
    private listeningTimerFlag;
    private connectionTipTimer;
    calls: Record<string, BridgeCall>;
    private connectionLost;
    private connectionLostReason;
    constructor(host: string, port: number, onConnect?: (() => void) | undefined, onDisconnect?: ((reason: string) => void) | undefined, closeConflictServer?: boolean | undefined);
    listen(opts?: {
        timeout?: number | false;
    }): Promise<void>;
    private connectionLostErrorMsg;
    private triggerCallResponseCallback;
    private emitCall;
    call<T = any>(method: string, args: any[], timeout?: number): Promise<T>;
    close(): Promise<void | undefined>;
}
