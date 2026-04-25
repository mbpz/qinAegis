import { BASIC_ENV_KEYS } from "./types.mjs";
const getBasicEnvValue = (key)=>{
    if (!BASIC_ENV_KEYS.includes(key)) throw new Error(`getBasicEnvValue with key ${key} is not supported.`);
    return process.env[key];
};
export { getBasicEnvValue };
