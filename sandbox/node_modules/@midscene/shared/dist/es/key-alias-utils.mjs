function kebabToCamel(str) {
    return str.replace(/-([a-z])/g, (_, letter)=>letter.toUpperCase());
}
function camelToKebab(str) {
    return str.replace(/[A-Z]/g, (letter)=>`-${letter.toLowerCase()}`).replace(/^-/, '');
}
function getKeyAliases(key) {
    return [
        ...new Set([
            key,
            kebabToCamel(key),
            camelToKebab(key)
        ])
    ];
}
function isRecord(value) {
    return 'object' == typeof value && null !== value && !Array.isArray(value);
}
export { camelToKebab, getKeyAliases, isRecord, kebabToCamel };
