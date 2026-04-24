export declare class IndexedDBManager {
    private dbPromise;
    private dbName;
    private version;
    private storeConfigs;
    constructor(dbName: string, version: number, storeConfigs: Array<{
        name: string;
        keyPath: string;
    }>);
    private initDB;
    private withTransaction;
    private promisifyRequest;
    put<T>(storeName: string, data: T): Promise<void>;
    get<T>(storeName: string, key: string): Promise<T | undefined>;
    getAll<T>(storeName: string, sortByTimestamp?: boolean): Promise<T[]>;
    clear(storeName: string): Promise<void>;
    delete(storeName: string, key: string): Promise<void>;
    count(storeName: string): Promise<number>;
    getDBPromise(): Promise<IDBDatabase>;
}
export declare const withErrorHandling: <T>(operation: () => Promise<T>, errorMessage: string, defaultValue?: T, onQuotaExceeded?: () => Promise<void>) => Promise<T | undefined>;
export declare const createCleanupFunction: <T extends {
    id: string;
    timestamp: number;
}>(dbManager: IndexedDBManager, storeName: string, maxItems: number) => () => Promise<void>;
