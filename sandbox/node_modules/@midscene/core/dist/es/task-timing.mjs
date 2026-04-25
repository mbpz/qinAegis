import { getDebug } from "@midscene/shared/logger";
const debugTiming = getDebug('task-timing');
function setTimingFieldOnce(timing, field) {
    if (!timing) return void debugTiming(`[warning] timing object missing, skip set. field=${field}`);
    const value = Date.now();
    const existingValue = timing[field];
    if (void 0 !== existingValue) return void debugTiming(`[warning] duplicate timing field set ignored. field=${field}, existing=${existingValue}, incoming=${value}`);
    timing[field] = value;
}
export { setTimingFieldOnce };

//# sourceMappingURL=task-timing.mjs.map