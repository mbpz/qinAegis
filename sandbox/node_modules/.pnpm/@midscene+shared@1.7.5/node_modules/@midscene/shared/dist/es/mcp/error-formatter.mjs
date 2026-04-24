function getErrorMessage(error) {
    if (error instanceof Error) return error.message;
    if (null == error) return String(error);
    if ('object' != typeof error) return String(error);
    const candidate = extractStringMessage(error);
    if (candidate) return candidate;
    try {
        return JSON.stringify(error);
    } catch  {
        return Object.prototype.toString.call(error);
    }
}
function extractStringMessage(error) {
    const anyError = error;
    if ('string' == typeof anyError.message && anyError.message) return anyError.message;
    if (anyError.error && 'string' == typeof anyError.error.message && anyError.error.message) return anyError.error.message;
    if (anyError.cause && 'string' == typeof anyError.cause.message && anyError.cause.message) return anyError.cause.message;
}
export { getErrorMessage };
