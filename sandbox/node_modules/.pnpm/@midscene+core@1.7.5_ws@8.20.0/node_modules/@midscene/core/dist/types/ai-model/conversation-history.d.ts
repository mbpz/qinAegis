import type { SubGoal } from '../types';
import type { ChatCompletionMessageParam } from 'openai/resources/index';
export interface ConversationHistoryOptions {
    initialMessages?: ChatCompletionMessageParam[];
}
export declare class ConversationHistory {
    private readonly messages;
    private subGoals;
    private memories;
    private historicalLogs;
    pendingFeedbackMessage: string;
    constructor(options?: ConversationHistoryOptions);
    resetPendingFeedbackMessageIfExists(): void;
    append(message: ChatCompletionMessageParam): void;
    seed(messages: ChatCompletionMessageParam[]): void;
    reset(): void;
    /**
     * Snapshot the conversation history, and replace the images with text if the number of images exceeds the limit.
     * @param maxImages - The maximum number of images to include in the snapshot. Undefined means no limit.
     * @returns The snapshot of the conversation history.
     */
    snapshot(maxImages?: number): ChatCompletionMessageParam[];
    get length(): number;
    [Symbol.iterator](): IterableIterator<ChatCompletionMessageParam>;
    toJSON(): ChatCompletionMessageParam[];
    /**
     * Set all sub-goals, replacing any existing ones.
     * Automatically marks the first pending goal as running.
     */
    setSubGoals(subGoals: SubGoal[]): void;
    /**
     * Merge sub-goals from update-plan-content.
     * Preserves existing descriptions when incoming description is empty.
     *
     * This handles compact XML updates like:
     * <sub-goal index="1" status="finished" />
     */
    mergeSubGoals(subGoals: SubGoal[]): void;
    /**
     * Update a single sub-goal by index.
     * Clears logs if status or description actually changes.
     * @returns true if the sub-goal was found and updated, false otherwise
     */
    updateSubGoal(index: number, updates: Partial<Omit<SubGoal, 'index'>>): boolean;
    /**
     * Mark the first pending sub-goal as running.
     * Clears logs since status changes.
     */
    markFirstPendingAsRunning(): void;
    /**
     * Mark a sub-goal as finished.
     * Automatically marks the next pending goal as running.
     * @returns true if the sub-goal was found and updated, false otherwise
     */
    markSubGoalFinished(index: number): boolean;
    /**
     * Mark all sub-goals as finished.
     * Clears logs for any goal whose status actually changes.
     */
    markAllSubGoalsFinished(): void;
    /**
     * Append a log entry to the currently running sub-goal.
     * The log describes an action performed while working on the sub-goal.
     */
    appendSubGoalLog(log: string): void;
    /**
     * Convert sub-goals to text representation.
     * Includes actions performed (logs) for the current sub-goal.
     */
    subGoalsToText(): string;
    /**
     * Append a log entry to the historical logs list.
     * Used in non-deepThink mode to track executed steps across planning rounds.
     */
    appendHistoricalLog(log: string): void;
    /**
     * Convert historical logs to text representation.
     * Provides context about previously executed steps to the model.
     */
    historicalLogsToText(): string;
    /**
     * Append a memory to the memories list
     */
    appendMemory(memory: string): void;
    /**
     * Get all memories
     */
    getMemories(): string[];
    /**
     * Convert memories to text representation
     */
    memoriesToText(): string;
    /**
     * Clear all memories
     */
    clearMemories(): void;
    /**
     * Compress the conversation history if it exceeds the threshold.
     * Removes the oldest messages and replaces them with a single placeholder message.
     * @param threshold - The number of messages that triggers compression.
     * @param keepCount - The number of recent messages to keep after compression.
     * @returns true if compression was performed, false otherwise.
     */
    compressHistory(threshold: number, keepCount: number): boolean;
}
