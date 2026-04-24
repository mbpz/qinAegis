function truncateText(text, maxLength = 150) {
    if (void 0 === text) return '';
    if ('object' == typeof text) text = JSON.stringify(text);
    if ('number' == typeof text) return text.toString();
    if ('string' == typeof text && text.length > maxLength) return `${text.slice(0, maxLength)}...`;
    if ('string' == typeof text) return text.trim();
    return '';
}
function trimAttributes(attributes, truncateTextLength) {
    const tailorAttributes = Object.keys(attributes).reduce((res, currentKey)=>{
        const attributeVal = attributes[currentKey];
        if ('style' === currentKey || 'htmlTagName' === currentKey || 'nodeType' === currentKey) return res;
        res[currentKey] = truncateText(attributeVal, truncateTextLength);
        return res;
    }, {});
    return tailorAttributes;
}
const nodeSizeThreshold = 4;
function descriptionOfTree(tree, truncateTextLength, filterNonTextContent = false, visibleOnly = true) {
    const attributesString = (kv)=>Object.entries(kv).map(([key, value])=>`${key}="${truncateText(value, truncateTextLength)}"`).join(' ');
    function buildContentTree(node, indent = 0, visibleOnly = true) {
        let before = '';
        let contentWithIndent = '';
        let after = '';
        let emptyNode = true;
        const indentStr = '  '.repeat(indent);
        let children = '';
        for(let i = 0; i < (node.children || []).length; i++){
            const childContent = buildContentTree(node.children[i], indent + 1, visibleOnly);
            if (childContent) children += `\n${childContent}`;
        }
        if (node.node && node.node.rect.width > nodeSizeThreshold && node.node.rect.height > nodeSizeThreshold && (!filterNonTextContent || filterNonTextContent && node.node.content) && (!visibleOnly || visibleOnly && node.node.isVisible)) {
            emptyNode = false;
            let nodeTypeString;
            nodeTypeString = node.node.attributes?.htmlTagName ? node.node.attributes.htmlTagName.replace(/[<>]/g, '') : node.node.attributes.nodeType.replace(/\sNode$/, '').toLowerCase();
            const rectAttribute = node.node.rect ? {
                left: node.node.rect.left,
                top: node.node.rect.top,
                width: node.node.rect.width,
                height: node.node.rect.height
            } : {};
            before = `<${nodeTypeString} id="${node.node.id}" ${attributesString(trimAttributes(node.node.attributes || {}, truncateTextLength))} ${attributesString(rectAttribute)}>`;
            const content = truncateText(node.node.content, truncateTextLength);
            contentWithIndent = content ? `\n${indentStr}  ${content}` : '';
            after = `</${nodeTypeString}>`;
        } else if (!filterNonTextContent) {
            if (!children.trim().startsWith('<>')) {
                before = '<>';
                contentWithIndent = '';
                after = '</>';
            }
        }
        if (emptyNode && !children.trim()) return '';
        const result = `${indentStr}${before}${contentWithIndent}${children}\n${indentStr}${after}`;
        if (result.trim()) return result;
        return '';
    }
    const result = buildContentTree(tree, 0, visibleOnly);
    return result.replace(/^\s*\n/gm, '');
}
function treeToList(tree) {
    const result = [];
    function dfs(node) {
        if (node.node) result.push(node.node);
        for (const child of node.children)dfs(child);
    }
    dfs(tree);
    return result;
}
function traverseTree(tree, onNode) {
    function dfs(node) {
        if (node.node) node.node = onNode(node.node);
        for (const child of node.children)dfs(child);
    }
    dfs(tree);
    return tree;
}
export { descriptionOfTree, traverseTree, treeToList, trimAttributes, truncateText };
