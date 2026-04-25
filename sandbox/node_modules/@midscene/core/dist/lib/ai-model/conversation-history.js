"use strict";
var __webpack_require__ = {};
(()=>{
    __webpack_require__.d = (exports1, definition)=>{
        for(var key in definition)if (__webpack_require__.o(definition, key) && !__webpack_require__.o(exports1, key)) Object.defineProperty(exports1, key, {
            enumerable: true,
            get: definition[key]
        });
    };
})();
(()=>{
    __webpack_require__.o = (obj, prop)=>Object.prototype.hasOwnProperty.call(obj, prop);
})();
(()=>{
    __webpack_require__.r = (exports1)=>{
        if ('undefined' != typeof Symbol && Symbol.toStringTag) Object.defineProperty(exports1, Symbol.toStringTag, {
            value: 'Module'
        });
        Object.defineProperty(exports1, '__esModule', {
            value: true
        });
    };
})();
var __webpack_exports__ = {};
__webpack_require__.r(__webpack_exports__);
__webpack_require__.d(__webpack_exports__, {
    ConversationHistory: ()=>ConversationHistory
});
function _define_property(obj, key, value) {
    if (key in obj) Object.defineProperty(obj, key, {
        value: value,
        enumerable: true,
        configurable: true,
        writable: true
    });
    else obj[key] = value;
    return obj;
}
var _computedKey;
_computedKey = Symbol.iterator;
let _computedKey1 = _computedKey;
class ConversationHistory {
    resetPendingFeedbackMessageIfExists() {
        if (this.pendingFeedbackMessage) this.pendingFeedbackMessage = '';
    }
    append(message) {
        this.messages.push(message);
    }
    seed(messages) {
        this.reset();
        messages.forEach((message)=>{
            this.append(message);
        });
    }
    reset() {
        this.messages.length = 0;
        this.memories.length = 0;
        this.subGoals.length = 0;
        this.historicalLogs.length = 0;
        this.pendingFeedbackMessage = '';
    }
    snapshot(maxImages) {
        if (void 0 === maxImages) return [
            ...this.messages
        ];
        const clonedMessages = structuredClone(this.messages);
        let imageCount = 0;
        for(let i = clonedMessages.length - 1; i >= 0; i--){
            const message = clonedMessages[i];
            const content = message.content;
            if (Array.isArray(content)) for(let j = 0; j < content.length; j++){
                const item = content[j];
                if ('image_url' === item.type) {
                    imageCount++;
                    if (imageCount > maxImages) content[j] = {
                        type: 'text',
                        text: '(image ignored due to size optimization)'
                    };
                }
            }
        }
        return clonedMessages;
    }
    get length() {
        return this.messages.length;
    }
    [_computedKey1]() {
        return this.messages[Symbol.iterator]();
    }
    toJSON() {
        return this.snapshot();
    }
    setSubGoals(subGoals) {
        this.subGoals = subGoals.map((goal)=>({
                ...goal
            }));
        this.markFirstPendingAsRunning();
    }
    mergeSubGoals(subGoals) {
        if (0 === this.subGoals.length) return void this.setSubGoals(subGoals);
        const existingByIndex = new Map(this.subGoals.map((goal)=>[
                goal.index,
                goal
            ]));
        const mergedSubGoals = subGoals.map((goal)=>{
            const existingGoal = existingByIndex.get(goal.index);
            const hasNonEmptyDescription = goal.description.trim().length > 0;
            if (!existingGoal && !hasNonEmptyDescription) return null;
            return {
                ...goal,
                description: hasNonEmptyDescription || !existingGoal ? goal.description : existingGoal.description
            };
        });
        const validSubGoals = mergedSubGoals.filter((goal)=>null !== goal);
        if (0 === validSubGoals.length) return;
        this.setSubGoals(validSubGoals);
    }
    updateSubGoal(index, updates) {
        const goal = this.subGoals.find((g)=>g.index === index);
        if (!goal) return false;
        let changed = false;
        if (void 0 !== updates.status && updates.status !== goal.status) {
            goal.status = updates.status;
            changed = true;
        }
        if (void 0 !== updates.description && updates.description !== goal.description) {
            goal.description = updates.description;
            changed = true;
        }
        if (changed) goal.logs = [];
        return true;
    }
    markFirstPendingAsRunning() {
        const firstPending = this.subGoals.find((g)=>'pending' === g.status);
        if (firstPending) {
            firstPending.status = 'running';
            firstPending.logs = [];
        }
    }
    markSubGoalFinished(index) {
        const result = this.updateSubGoal(index, {
            status: 'finished'
        });
        if (result) this.markFirstPendingAsRunning();
        return result;
    }
    markAllSubGoalsFinished() {
        for (const goal of this.subGoals){
            if ('finished' !== goal.status) goal.logs = [];
            goal.status = 'finished';
        }
    }
    appendSubGoalLog(log) {
        if (!log) return;
        const runningGoal = this.subGoals.find((g)=>'running' === g.status);
        if (runningGoal) {
            if (!runningGoal.logs) runningGoal.logs = [];
            runningGoal.logs.push(log);
        }
    }
    subGoalsToText() {
        if (0 === this.subGoals.length) return '';
        const lines = this.subGoals.map((goal)=>`${goal.index}. ${goal.description} (${goal.status})`);
        const currentGoal = this.subGoals.find((goal)=>'running' === goal.status) || this.subGoals.find((goal)=>'pending' === goal.status);
        let currentGoalText = '';
        if (currentGoal) {
            currentGoalText = `\nCurrent sub-goal is: ${currentGoal.description}`;
            if (currentGoal.logs && currentGoal.logs.length > 0) {
                const logLines = currentGoal.logs.map((log)=>`- ${log}`).join('\n');
                currentGoalText += `\nActions performed for current sub-goal:\n${logLines}`;
            }
        }
        return `Sub-goals:\n${lines.join('\n')}${currentGoalText}`;
    }
    appendHistoricalLog(log) {
        if (log) this.historicalLogs.push(log);
    }
    historicalLogsToText() {
        if (0 === this.historicalLogs.length) return '';
        const logLines = this.historicalLogs.map((log)=>`- ${log}`).join('\n');
        return `Here are the steps that have been executed:\n${logLines}`;
    }
    appendMemory(memory) {
        if (memory) this.memories.push(memory);
    }
    getMemories() {
        return [
            ...this.memories
        ];
    }
    memoriesToText() {
        if (0 === this.memories.length) return '';
        return `Memories from previous steps:\n---\n${this.memories.join('\n---\n')}\n`;
    }
    clearMemories() {
        this.memories.length = 0;
    }
    compressHistory(threshold, keepCount) {
        if (this.messages.length <= threshold) return false;
        const omittedCount = this.messages.length - keepCount;
        const omittedPlaceholder = {
            role: 'user',
            content: `(${omittedCount} previous conversation messages have been omitted)`
        };
        const recentMessages = this.messages.slice(-keepCount);
        this.messages.length = 0;
        this.messages.push(omittedPlaceholder);
        for (const msg of recentMessages)this.messages.push(msg);
        return true;
    }
    constructor(options){
        _define_property(this, "messages", []);
        _define_property(this, "subGoals", []);
        _define_property(this, "memories", []);
        _define_property(this, "historicalLogs", []);
        _define_property(this, "pendingFeedbackMessage", void 0);
        if (options?.initialMessages?.length) this.seed(options.initialMessages);
        this.pendingFeedbackMessage = '';
    }
}
exports.ConversationHistory = __webpack_exports__.ConversationHistory;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "ConversationHistory"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=conversation-history.js.map