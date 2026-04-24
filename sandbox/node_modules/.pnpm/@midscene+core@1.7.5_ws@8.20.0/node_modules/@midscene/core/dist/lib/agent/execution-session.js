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
    ExecutionSession: ()=>ExecutionSession
});
const external_task_runner_js_namespaceObject = require("../task-runner.js");
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
        this.runner = new external_task_runner_js_namespaceObject.TaskRunner(name, contextProvider, options);
    }
}
exports.ExecutionSession = __webpack_exports__.ExecutionSession;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "ExecutionSession"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=execution-session.js.map