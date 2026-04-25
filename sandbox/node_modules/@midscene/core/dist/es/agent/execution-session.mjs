import { TaskRunner } from "../task-runner.mjs";
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
class ExecutionSession {
    async append(tasks, options) {
        await this.runner.append(tasks, options);
    }
    async appendAndRun(tasks, options) {
        return this.runner.appendAndFlush(tasks, options);
    }
    async run(options) {
        return this.runner.flush(options);
    }
    isInErrorState() {
        return this.runner.isInErrorState();
    }
    latestErrorTask() {
        return this.runner.latestErrorTask();
    }
    appendErrorPlan(errorMsg) {
        return this.runner.appendErrorPlan(errorMsg);
    }
    getRunner() {
        return this.runner;
    }
    constructor(name, contextProvider, options){
        _define_property(this, "runner", void 0);
        this.runner = new TaskRunner(name, contextProvider, options);
    }
}
export { ExecutionSession };

//# sourceMappingURL=execution-session.mjs.map