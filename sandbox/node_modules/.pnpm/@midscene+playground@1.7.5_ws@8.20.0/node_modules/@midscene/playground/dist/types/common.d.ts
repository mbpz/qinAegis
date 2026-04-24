import type { DeviceAction } from '@midscene/core';
import type { ExecutionOptions, FormValue, PlaygroundAgent, ValidationResult } from './types';
export declare const dataExtractionAPIs: string[];
export declare const validationAPIs: string[];
export declare const noReplayAPIs: string[];
export declare const formatErrorMessage: (e: any) => string;
export declare function parseStructuredParams(action: DeviceAction<unknown>, params: Record<string, unknown>, options?: ExecutionOptions): Promise<unknown[]>;
export declare function validateStructuredParams(value: FormValue, action: DeviceAction<unknown> | undefined): ValidationResult;
export declare function executeAction(activeAgent: PlaygroundAgent, actionType: string, actionSpace: DeviceAction<unknown>[], value: FormValue, options: ExecutionOptions): Promise<unknown>;
