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
    distance: ()=>distance,
    distanceThreshold: ()=>distanceThreshold,
    extractXMLTag: ()=>extractXMLTag,
    parseMarkFinishedIndexes: ()=>parseMarkFinishedIndexes,
    parseSubGoalsFromXML: ()=>parseSubGoalsFromXML
});
function extractXMLTag(xmlString, tagName) {
    const lowerXmlString = xmlString.toLowerCase();
    const lowerTagName = tagName.toLowerCase();
    const closeTag = `</${lowerTagName}>`;
    const openTag = `<${lowerTagName}>`;
    const lastCloseIndex = lowerXmlString.lastIndexOf(closeTag);
    if (-1 === lastCloseIndex) {
        const lastOpenIndex = lowerXmlString.lastIndexOf(openTag);
        if (-1 === lastOpenIndex) return;
        const contentStart = lastOpenIndex + openTag.length;
        const remaining = xmlString.substring(contentStart);
        const nextTagIndex = remaining.indexOf('<');
        const content = -1 === nextTagIndex ? remaining : remaining.substring(0, nextTagIndex);
        return content.trim();
    }
    const searchArea = lowerXmlString.substring(0, lastCloseIndex);
    const lastOpenIndex = searchArea.lastIndexOf(openTag);
    if (-1 === lastOpenIndex) return;
    const contentStart = lastOpenIndex + openTag.length;
    const contentEnd = lastCloseIndex;
    const content = xmlString.substring(contentStart, contentEnd);
    return content.trim();
}
function parseSubGoalsFromXML(xmlContent) {
    const subGoals = [];
    const regex = /<sub-goal\s+index="(\d+)"\s+status="(pending|finished)"(?:\s*\/>|>([\s\S]*?)<\/sub-goal>)/gi;
    let match;
    match = regex.exec(xmlContent);
    while(null !== match){
        const index = Number.parseInt(match[1], 10);
        const status = match[2];
        const description = match[3]?.trim() || '';
        subGoals.push({
            index,
            status,
            description
        });
        match = regex.exec(xmlContent);
    }
    return subGoals;
}
function parseMarkFinishedIndexes(xmlContent) {
    const indexes = [];
    const regex = /<sub-goal\s+index="(\d+)"\s+status="finished"\s*\/>/gi;
    let match;
    match = regex.exec(xmlContent);
    while(null !== match){
        indexes.push(Number.parseInt(match[1], 10));
        match = regex.exec(xmlContent);
    }
    return indexes;
}
const distanceThreshold = 16;
function distance(point1, point2) {
    return Math.sqrt((point1.x - point2.x) ** 2 + (point1.y - point2.y) ** 2);
}
exports.distance = __webpack_exports__.distance;
exports.distanceThreshold = __webpack_exports__.distanceThreshold;
exports.extractXMLTag = __webpack_exports__.extractXMLTag;
exports.parseMarkFinishedIndexes = __webpack_exports__.parseMarkFinishedIndexes;
exports.parseSubGoalsFromXML = __webpack_exports__.parseSubGoalsFromXML;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "distance",
    "distanceThreshold",
    "extractXMLTag",
    "parseMarkFinishedIndexes",
    "parseSubGoalsFromXML"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=util.js.map