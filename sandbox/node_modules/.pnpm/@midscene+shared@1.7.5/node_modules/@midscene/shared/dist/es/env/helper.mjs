const maskKey = (key, maskChar = '*')=>{
    if ('string' != typeof key || 0 === key.length) return key;
    const prefixLen = 3;
    const suffixLen = 3;
    const keepLength = prefixLen + suffixLen;
    if (key.length <= keepLength) return key;
    const prefix = key.substring(0, prefixLen);
    const suffix = key.substring(key.length - suffixLen);
    const maskLength = key.length - keepLength;
    const mask = maskChar.repeat(maskLength);
    return `${prefix}${mask}${suffix}`;
};
const maskConfig = (config)=>Object.fromEntries(Object.entries(config).map(([key, value])=>{
        if (!value) return [
            key,
            value
        ];
        if ('string' == typeof value && /key/i.test(key)) return [
            key,
            maskKey(value)
        ];
        if ('object' == typeof value) {
            const valueStr = JSON.stringify(value);
            if (/key/i.test(valueStr)) return [
                key,
                maskKey(valueStr)
            ];
        }
        return [
            key,
            value
        ];
    }));
const parseJson = (key, value)=>{
    if (value) try {
        return JSON.parse(value);
    } catch (e) {
        throw new Error(`Failed to parse ${key} as a JSON. ${e.message}`, {
            cause: e
        });
    }
};
export { maskConfig, parseJson };
