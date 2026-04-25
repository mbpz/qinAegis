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
class CLIError extends Error {
    constructor(message, exitCode = 1){
        super(message), _define_property(this, "exitCode", void 0), this.exitCode = exitCode;
    }
}
function reportCLIError(error, log = console.error) {
    if (error instanceof CLIError) {
        log(error.message);
        return error.exitCode;
    }
    log(error);
    return 1;
}
export { CLIError, reportCLIError };
