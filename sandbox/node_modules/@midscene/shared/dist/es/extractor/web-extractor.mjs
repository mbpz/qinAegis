import { CONTAINER_MINI_HEIGHT, CONTAINER_MINI_WIDTH, NodeType } from "../constants/index.mjs";
import { isAElement, isButtonElement, isContainerElement, isFormElement, isImgElement, isTextElement } from "./dom-util.mjs";
import { descriptionOfTree } from "./tree.mjs";
import { elementRect, getNodeAttributes, getPseudoElementContent, getRect, getTopDocument, logger, midsceneGenerateHash, setDebugMode } from "./util.mjs";
let indexId = 0;
function tagNameOfNode(node) {
    let tagName = '';
    if (node instanceof HTMLElement) tagName = node.tagName?.toLowerCase();
    else {
        const parentElement = node.parentElement;
        if (parentElement && parentElement instanceof HTMLElement) tagName = parentElement.tagName?.toLowerCase();
    }
    return tagName ? `<${tagName}>` : '';
}
function collectElementInfo(node, currentWindow, currentDocument, baseZoom = 1, basePoint = {
    left: 0,
    top: 0
}, isContainer = false) {
    const rect = elementRect(node, currentWindow, currentDocument, baseZoom);
    if (!rect) return null;
    if (rect.width < CONTAINER_MINI_WIDTH || rect.height < CONTAINER_MINI_HEIGHT) return null;
    if (0 !== basePoint.left || 0 !== basePoint.top) {
        rect.left += basePoint.left;
        rect.top += basePoint.top;
    }
    if (rect.height >= window.innerHeight && rect.width >= window.innerWidth) return null;
    if (isFormElement(node)) {
        const attributes = getNodeAttributes(node, currentWindow);
        let valueContent = attributes.value || attributes.placeholder || node.textContent || '';
        const nodeHashId = midsceneGenerateHash(node, valueContent, rect);
        const tagName = node.tagName.toLowerCase();
        if ('select' === node.tagName.toLowerCase()) {
            const selectedOption = node.options[node.selectedIndex];
            valueContent = selectedOption?.textContent || '';
        }
        if (('input' === node.tagName.toLowerCase() || 'textarea' === node.tagName.toLowerCase()) && node.value) valueContent = node.value;
        const elementInfo = {
            id: nodeHashId,
            nodeHashId,
            nodeType: NodeType.FORM_ITEM,
            indexId: indexId++,
            attributes: {
                ...attributes,
                htmlTagName: `<${tagName}>`,
                nodeType: NodeType.FORM_ITEM
            },
            content: valueContent.trim(),
            rect,
            center: [
                Math.round(rect.left + rect.width / 2),
                Math.round(rect.top + rect.height / 2)
            ],
            zoom: rect.zoom,
            isVisible: rect.isVisible
        };
        return elementInfo;
    }
    if (isButtonElement(node)) {
        const rect = mergeElementAndChildrenRects(node, currentWindow, currentDocument, baseZoom);
        if (!rect) return null;
        const attributes = getNodeAttributes(node, currentWindow);
        const pseudo = getPseudoElementContent(node, currentWindow);
        const content = node.innerText || pseudo.before || pseudo.after || '';
        const nodeHashId = midsceneGenerateHash(node, content, rect);
        const elementInfo = {
            id: nodeHashId,
            indexId: indexId++,
            nodeHashId,
            nodeType: NodeType.BUTTON,
            attributes: {
                ...attributes,
                htmlTagName: tagNameOfNode(node),
                nodeType: NodeType.BUTTON
            },
            content,
            rect,
            center: [
                Math.round(rect.left + rect.width / 2),
                Math.round(rect.top + rect.height / 2)
            ],
            zoom: rect.zoom,
            isVisible: rect.isVisible
        };
        return elementInfo;
    }
    if (isImgElement(node)) {
        const attributes = getNodeAttributes(node, currentWindow);
        const nodeHashId = midsceneGenerateHash(node, '', rect);
        const elementInfo = {
            id: nodeHashId,
            indexId: indexId++,
            nodeHashId,
            attributes: {
                ...attributes,
                ...node.nodeName?.toLowerCase() === 'svg' ? {
                    svgContent: 'true'
                } : {},
                nodeType: NodeType.IMG,
                htmlTagName: tagNameOfNode(node)
            },
            nodeType: NodeType.IMG,
            content: '',
            rect,
            center: [
                Math.round(rect.left + rect.width / 2),
                Math.round(rect.top + rect.height / 2)
            ],
            zoom: rect.zoom,
            isVisible: rect.isVisible
        };
        return elementInfo;
    }
    if (isTextElement(node)) {
        const text = node.textContent?.trim().replace(/\n+/g, ' ');
        if (!text) return null;
        const attributes = getNodeAttributes(node, currentWindow);
        const attributeKeys = Object.keys(attributes);
        if (!text.trim() && 0 === attributeKeys.length) return null;
        const nodeHashId = midsceneGenerateHash(node, text, rect);
        const elementInfo = {
            id: nodeHashId,
            indexId: indexId++,
            nodeHashId,
            nodeType: NodeType.TEXT,
            attributes: {
                ...attributes,
                nodeType: NodeType.TEXT,
                htmlTagName: tagNameOfNode(node)
            },
            center: [
                Math.round(rect.left + rect.width / 2),
                Math.round(rect.top + rect.height / 2)
            ],
            content: text,
            rect,
            zoom: rect.zoom,
            isVisible: rect.isVisible
        };
        return elementInfo;
    }
    if (isAElement(node)) {
        const attributes = getNodeAttributes(node, currentWindow);
        const pseudo = getPseudoElementContent(node, currentWindow);
        const content = node.innerText || pseudo.before || pseudo.after || '';
        const nodeHashId = midsceneGenerateHash(node, content, rect);
        const elementInfo = {
            id: nodeHashId,
            indexId: indexId++,
            nodeHashId,
            nodeType: NodeType.A,
            attributes: {
                ...attributes,
                htmlTagName: tagNameOfNode(node),
                nodeType: NodeType.A
            },
            content,
            rect,
            center: [
                Math.round(rect.left + rect.width / 2),
                Math.round(rect.top + rect.height / 2)
            ],
            zoom: rect.zoom,
            isVisible: rect.isVisible
        };
        return elementInfo;
    }
    if (isContainerElement(node) || isContainer) {
        const attributes = getNodeAttributes(node, currentWindow);
        const nodeHashId = midsceneGenerateHash(node, '', rect);
        const elementInfo = {
            id: nodeHashId,
            nodeHashId,
            indexId: indexId++,
            nodeType: NodeType.CONTAINER,
            attributes: {
                ...attributes,
                nodeType: NodeType.CONTAINER,
                htmlTagName: tagNameOfNode(node)
            },
            content: '',
            rect,
            center: [
                Math.round(rect.left + rect.width / 2),
                Math.round(rect.top + rect.height / 2)
            ],
            zoom: rect.zoom,
            isVisible: rect.isVisible
        };
        return elementInfo;
    }
    return null;
}
function extractTextWithPosition(initNode, debugMode = false) {
    const elementNode = extractTreeNode(initNode, debugMode);
    const elementInfoArray = [];
    function dfsTopChildren(node) {
        if (node.node) elementInfoArray.push(node.node);
        for(let i = 0; i < node.children.length; i++)dfsTopChildren(node.children[i]);
    }
    dfsTopChildren({
        children: elementNode.children,
        node: elementNode.node
    });
    return elementInfoArray;
}
function extractTreeNodeAsString(initNode, visibleOnly = false, debugMode = false) {
    const elementNode = extractTreeNode(initNode, debugMode);
    return descriptionOfTree(elementNode, void 0, false, visibleOnly);
}
function extractTreeNode(initNode, debugMode = false) {
    setDebugMode(debugMode);
    indexId = 0;
    const topDocument = getTopDocument();
    const startNode = initNode || topDocument;
    const topChildren = [];
    function dfs(node, currentWindow, currentDocument, baseZoom = 1, basePoint = {
        left: 0,
        top: 0
    }) {
        if (!node) return null;
        if (node.nodeType && 10 === node.nodeType) return null;
        const elementInfo = collectElementInfo(node, currentWindow, currentDocument, baseZoom, basePoint);
        if (node instanceof currentWindow.HTMLIFrameElement) {
            if (node.contentWindow && node.contentWindow) return null;
        }
        const nodeInfo = {
            node: elementInfo,
            children: []
        };
        if (elementInfo?.nodeType === NodeType.BUTTON || elementInfo?.nodeType === NodeType.IMG || elementInfo?.nodeType === NodeType.TEXT || elementInfo?.nodeType === NodeType.FORM_ITEM || elementInfo?.nodeType === NodeType.CONTAINER) return nodeInfo;
        const rect = getRect(node, baseZoom, currentWindow);
        for(let i = 0; i < node.childNodes.length; i++){
            logger('will dfs', node.childNodes[i]);
            const childNodeInfo = dfs(node.childNodes[i], currentWindow, currentDocument, rect.zoom, basePoint);
            if (Array.isArray(childNodeInfo)) nodeInfo.children.push(...childNodeInfo);
            else if (childNodeInfo) nodeInfo.children.push(childNodeInfo);
        }
        if (null === nodeInfo.node) {
            if (0 === nodeInfo.children.length) return null;
            return nodeInfo.children;
        }
        return nodeInfo;
    }
    const rootNodeInfo = dfs(startNode, window, document, 1, {
        left: 0,
        top: 0
    });
    if (Array.isArray(rootNodeInfo)) topChildren.push(...rootNodeInfo);
    else if (rootNodeInfo) topChildren.push(rootNodeInfo);
    if (startNode === topDocument) {
        const iframes = document.querySelectorAll('iframe');
        for(let i = 0; i < iframes.length; i++){
            const iframe = iframes[i];
            if (iframe.contentDocument && iframe.contentWindow) {
                const iframeInfo = collectElementInfo(iframe, window, document, 1);
                if (iframeInfo) {
                    const iframeChildren = dfs(iframe.contentDocument.body, iframe.contentWindow, iframe.contentDocument, 1, {
                        left: iframeInfo.rect.left,
                        top: iframeInfo.rect.top
                    });
                    if (Array.isArray(iframeChildren)) topChildren.push(...iframeChildren);
                    else if (iframeChildren) topChildren.push(iframeChildren);
                }
            }
        }
    }
    return {
        node: null,
        children: topChildren
    };
}
function mergeElementAndChildrenRects(node, currentWindow, currentDocument, baseZoom = 1) {
    const selfRect = elementRect(node, currentWindow, currentDocument, baseZoom);
    if (!selfRect) return null;
    let minLeft = selfRect.left;
    let minTop = selfRect.top;
    let maxRight = selfRect.left + selfRect.width;
    let maxBottom = selfRect.top + selfRect.height;
    function traverse(child) {
        for(let i = 0; i < child.childNodes.length; i++){
            const sub = child.childNodes[i];
            if (1 === sub.nodeType) {
                const rect = elementRect(sub, currentWindow, currentDocument, baseZoom);
                if (rect) {
                    minLeft = Math.min(minLeft, rect.left);
                    minTop = Math.min(minTop, rect.top);
                    maxRight = Math.max(maxRight, rect.left + rect.width);
                    maxBottom = Math.max(maxBottom, rect.top + rect.height);
                }
                traverse(sub);
            }
        }
    }
    traverse(node);
    return {
        ...selfRect,
        left: minLeft,
        top: minTop,
        width: maxRight - minLeft,
        height: maxBottom - minTop
    };
}
export { collectElementInfo, extractTextWithPosition, extractTreeNode, extractTreeNodeAsString, mergeElementAndChildrenRects };
