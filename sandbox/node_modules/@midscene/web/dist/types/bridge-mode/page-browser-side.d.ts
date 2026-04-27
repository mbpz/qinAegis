import ChromeExtensionProxyPage from '../chrome-extension/page';
import type { ChromePageDestroyOptions } from '../web-page';
import { type BridgeConnectTabOptions } from './common';
import { BridgeClient } from './io-client';
export declare class ExtensionBridgePageBrowserSide extends ChromeExtensionProxyPage {
    serverEndpoint?: string | undefined;
    onDisconnect: () => void;
    onLogMessage: (message: string, type: 'log' | 'status') => void;
    onConnectionRequest?: (() => Promise<boolean>) | undefined;
    bridgeClient: BridgeClient | null;
    private destroyOptions?;
    private newlyCreatedTabIds;
    private confirmationPromise;
    constructor(serverEndpoint?: string | undefined, onDisconnect?: () => void, onLogMessage?: (message: string, type: 'log' | 'status') => void, forceSameTabNavigation?: boolean, onConnectionRequest?: (() => Promise<boolean>) | undefined);
    private setupBridgeClient;
    connect(): Promise<void>;
    connectNewTabWithUrl(url: string, options?: BridgeConnectTabOptions): Promise<void>;
    connectCurrentTab(options?: BridgeConnectTabOptions): Promise<void>;
    setDestroyOptions(options: ChromePageDestroyOptions): Promise<void>;
    destroy(): Promise<void>;
}
