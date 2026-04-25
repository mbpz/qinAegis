import { BASIC_ENV_KEYS } from './types';
/**
 * get basic env value from process.env
 * use a single file to avoid circular dependency
 */
export declare const getBasicEnvValue: (key: (typeof BASIC_ENV_KEYS)[number]) => string | undefined;
