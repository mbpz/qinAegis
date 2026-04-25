import { getKeyAliases, isRecord } from "../key-alias-utils.mjs";
function readAliasedValue(args, key) {
    for (const alias of getKeyAliases(key))if (alias in args) return args[alias];
}
function readNamespacedArg(args, namespace, key) {
    const namespacedArgs = readAliasedValue(args, namespace);
    if (isRecord(namespacedArgs)) {
        const nestedValue = readAliasedValue(namespacedArgs, key);
        if (void 0 !== nestedValue) return nestedValue;
    }
    const dottedValue = readAliasedValue(args, `${namespace}.${key}`);
    if (void 0 !== dottedValue) return dottedValue;
    const directValue = readAliasedValue(args, key);
    if (void 0 !== directValue) return directValue;
}
function extractNamespacedArgs(args, namespace, keys) {
    const extracted = {};
    for (const key of keys){
        const value = readNamespacedArg(args, namespace, key);
        if (void 0 !== value) extracted[key] = value;
    }
    return Object.keys(extracted).length > 0 ? extracted : void 0;
}
function sanitizeNamespacedArgs(args, namespace, keys) {
    const excludedKeys = new Set(getKeyAliases(namespace));
    for (const key of keys){
        for (const alias of getKeyAliases(key))excludedKeys.add(alias);
        for (const alias of getKeyAliases(`${namespace}.${key}`))excludedKeys.add(alias);
    }
    return Object.fromEntries(Object.entries(args).filter(([key])=>!excludedKeys.has(key)));
}
function createNamespacedInitArgSchema(namespace, shape) {
    return Object.fromEntries(Object.entries(shape).map(([key, value])=>[
            `${namespace}.${key}`,
            value
        ]));
}
export { createNamespacedInitArgSchema, extractNamespacedArgs, sanitizeNamespacedArgs };
