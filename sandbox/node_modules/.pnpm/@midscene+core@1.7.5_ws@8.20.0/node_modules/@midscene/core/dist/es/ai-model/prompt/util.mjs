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
export { distance, distanceThreshold, extractXMLTag, parseMarkFinishedIndexes, parseSubGoalsFromXML };

//# sourceMappingURL=util.mjs.map