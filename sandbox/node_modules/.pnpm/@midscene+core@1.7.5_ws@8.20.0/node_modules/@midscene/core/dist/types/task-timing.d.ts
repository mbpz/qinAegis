import type { ExecutionTask } from './types';
type ExecutionTaskTiming = NonNullable<ExecutionTask['timing']>;
type NumericTimingField = {
    [K in keyof ExecutionTaskTiming]-?: ExecutionTaskTiming[K] extends number | undefined ? K : never;
}[keyof ExecutionTaskTiming];
export type TimingSettableField = Exclude<NumericTimingField, 'start' | 'end' | 'cost'>;
export declare function setTimingFieldOnce(timing: ExecutionTaskTiming | undefined, field: TimingSettableField): void;
export {};
