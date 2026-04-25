import type { Agent } from '../agent/agent';
import type { FreeFn, MidsceneYamlScript, MidsceneYamlScriptEnv, ScriptPlayerStatusValue, ScriptPlayerTaskStatus } from '../types';
export declare class ScriptPlayer<T extends MidsceneYamlScriptEnv> {
    private script;
    private setupAgent;
    onTaskStatusChange?: ((taskStatus: ScriptPlayerTaskStatus) => void) | undefined;
    currentTaskIndex?: number;
    taskStatusList: ScriptPlayerTaskStatus[];
    status: ScriptPlayerStatusValue;
    reportFile?: string | null;
    result: Record<string, any>;
    private unnamedResultIndex;
    output?: string | null;
    unstableLogContent?: string | null;
    errorInSetup?: Error;
    private interfaceAgent;
    agentStatusTip?: string;
    target?: MidsceneYamlScriptEnv;
    private actionSpace;
    private scriptPath?;
    constructor(script: MidsceneYamlScript, setupAgent: (platform: T) => Promise<{
        agent: Agent;
        freeFn: FreeFn[];
    }>, onTaskStatusChange?: ((taskStatus: ScriptPlayerTaskStatus) => void) | undefined, scriptPath?: string);
    private setResult;
    private setPlayerStatus;
    private notifyCurrentTaskStatusChange;
    private setTaskStatus;
    private setTaskIndex;
    private flushResult;
    private flushUnstableLogContent;
    playTask(taskStatus: ScriptPlayerTaskStatus, agent: Agent): Promise<void>;
    run(): Promise<void>;
}
