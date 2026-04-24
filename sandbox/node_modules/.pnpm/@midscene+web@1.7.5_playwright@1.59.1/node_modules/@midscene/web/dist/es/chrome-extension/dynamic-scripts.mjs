import node_fs from "node:fs";
import { ifInBrowser, ifInWorker } from "@midscene/shared/utils";
let scriptFileContentCache = null;
const getHtmlElementScript = async ()=>{
    const scriptFileToRetrieve = chrome.runtime.getURL("scripts/htmlElement.js");
    if (scriptFileContentCache) return scriptFileContentCache;
    if (ifInBrowser || ifInWorker) {
        const script = await fetch(scriptFileToRetrieve);
        scriptFileContentCache = await script.text();
        return scriptFileContentCache;
    }
    return node_fs.readFileSync(scriptFileToRetrieve, 'utf8');
};
let waterFlowScriptFileContentCache = null;
const injectWaterFlowAnimation = async ()=>{
    const waterFlowScriptFileToRetrieve = chrome.runtime.getURL("scripts/water-flow.js");
    if (waterFlowScriptFileContentCache) return waterFlowScriptFileContentCache;
    if (ifInBrowser || ifInWorker) {
        const script = await fetch(waterFlowScriptFileToRetrieve);
        waterFlowScriptFileContentCache = await script.text();
        return waterFlowScriptFileContentCache;
    }
    return node_fs.readFileSync(waterFlowScriptFileToRetrieve, 'utf8');
};
let stopWaterFlowScriptFileContentCache = null;
const injectStopWaterFlowAnimation = async ()=>{
    const stopWaterFlowScriptFileToRetrieve = chrome.runtime.getURL("scripts/stop-water-flow.js");
    if (stopWaterFlowScriptFileContentCache) return stopWaterFlowScriptFileContentCache;
    if (ifInBrowser || ifInWorker) {
        const script = await fetch(stopWaterFlowScriptFileToRetrieve);
        stopWaterFlowScriptFileContentCache = await script.text();
        return stopWaterFlowScriptFileContentCache;
    }
    return node_fs.readFileSync(stopWaterFlowScriptFileToRetrieve, 'utf8');
};
export { getHtmlElementScript, injectStopWaterFlowAnimation, injectWaterFlowAnimation };

//# sourceMappingURL=dynamic-scripts.mjs.map