import { ifInNode } from "../utils.mjs";
async function getSharp() {
    if (!ifInNode) throw new Error('Sharp is only available in Node.js environment');
    try {
        const sharp = await import("sharp");
        return sharp.default;
    } catch (error) {
        throw new Error(`Failed to load sharp module: ${error instanceof Error ? error.message : String(error)}`);
    }
}
export { getSharp as default };
