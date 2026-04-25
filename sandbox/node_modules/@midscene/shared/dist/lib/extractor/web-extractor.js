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
    mergeElementAndChildrenRects: ()=>mergeElementAndChildrenRects,
    extractTreeNode: ()=>extractTreeNode,
    extractTreeNodeAsString: ()=>extractTreeNodeAsString,
    collectElementInfo: ()=>collectElementInfo,
    extractTextWithPosition: ()=>extractTextWithPosition
});
const index_js_namespaceObject = require("../constants/index.js");
const external_dom_util_js_namespaceObject = require("./dom-util.js");
const external_tree_js_namespaceObject = require("./tree.js");
const external_util_js_namespaceObject = require("./util.js");
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
    const rect = (0, external_util_js_namespaceObject.elementRect)(node, currentWindow, currentDocument, baseZoom);
    if (!rect) return null;
    if (rect.width < index_js_namespaceObject.CONTAINER_MINI_WIDTH || rect.height < index_js_namespaceObject.CONTAINER_MINI_HEIGHT) return null;
    if (0 !== basePoint.left || 0 !== basePoint.top) {
        rect.left += basePoint.left;
        rect.top += basePoint.top;
    }
    if (rect.height >= window.innerHeight && rect.width >= window.innerWidth) return null;
    if ((0, external_dom_util_js_namespaceObject.isFormElement)(node)) {
        const attributes = (0, external_util_js_namespaceObject.getNodeAttributes)(node, currentWindow);
        let valueContent = attributes.value || attributes.placeholder || node.textContent || '';
        const nodeHashId = (0, external_util_js_namespaceObject.midsceneGenerateHash)(node, valueContent, rect);
        const tagName = node.tagName.toLowerCase();
        if ('select' === node.tagName.toLowerCase()) {
            const selectedOption = node.options[node.selectedIndex];
            valueContent = selectedOption?.textContent || '';
        }
        if (('input' === node.tagName.toLowerCase() || 'textarea' === node.tagName.toLowerCase()) && node.value) valueContent = node.value;
        const elementInfo = {
            id: nodeHashId,
            nodeHashId,
            nodeType: index_js_namespaceObject.NodeType.FORM_ITEM,
            indexId: indexId++,
            attributes: {
                ...attributes,
                htmlTagName: `<${tagName}>`,
                nodeType: index_js_namespaceObject.NodeType.FORM_ITEM
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
    if ((0, external_dom_util_js_namespaceObject.isButtonElement)(node)) {
        const rect = mergeElementAndChildrenRects(node, currentWindow, currentDocument, baseZoom);
        if (!rect) return null;
        const attributes = (0, external_util_js_namespaceObject.getNodeAttributes)(node, currentWindow);
        const pseudo = (0, external_util_js_namespaceObject.getPseudoElementContent)(node, currentWindow);
        const content = node.innerText || pseudo.before || pseudo.after || '';
        const nodeHashId = (0, external_util_js_namespaceObject.midsceneGenerateHash)(node, content, rect);
        const elementInfo = {
            id: nodeHashId,
            indexId: indexId++,
            nodeHashId,
            nodeType: index_js_namespaceObject.NodeType.BUTTON,
            attributes: {
                ...attributes,
                htmlTagName: tagNameOfNode(node),
                nodeType: index_js_namespaceObject.NodeType.BUTTON
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
    if ((0, external_dom_util_js_namespaceObject.isImgElement)(node)) {
        const attributes = (0, external_util_js_namespaceObject.getNodeAttributes)(node, currentWindow);
        const nodeHashId = (0, external_util_js_namespaceObject.midsceneGenerateHash)(node, '', rect);
        const elementInfo = {
            id: nodeHashId,
            indexId: indexId++,
            nodeHashId,
            attributes: {
                ...attributes,
                ...node.nodeName?.toLowerCase() === 'svg' ? {
                    svgContent: 'true'
                } : {},
                nodeType: index_js_namespaceObject.NodeType.IMG,
                htmlTagName: tagNameOfNode(node)
            },
            nodeType: index_js_namespaceObject.NodeType.IMG,
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
    if ((0, external_dom_util_js_namespaceObject.isTextElement)(node)) {
        const text = node.textContent?.trim().replace(/\n+/g, ' ');
        if (!text) return null;
        const attributes = (0, external_util_js_namespaceObject.getNodeAttributes)(node, currentWindow);
        const attributeKeys = Object.keys(attributes);
        if (!text.trim() && 0 === attributeKeys.length) return null;
        const nodeHashId = (0, external_util_js_namespaceObject.midsceneGenerateHash)(node, text, rect);
        const elementInfo = {
            id: nodeHashId,
            indexId: indexId++,
            nodeHashId,
            nodeType: index_js_namespaceObject.NodeType.TEXT,
            attributes: {
                ...attributes,
                nodeType: index_js_namespaceObject.NodeType.TEXT,
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
    if ((0, external_dom_util_js_namespaceObject.isAElement)(node)) {
        const attributes = (0, external_util_js_namespaceObject.getNodeAttributes)(node, currentWindow);
        const pseudo = (0, external_util_js_namespaceObject.getPseudoElementContent)(node, currentWindow);
        const content = node.innerText || pseudo.before || pseudo.after || '';
        const nodeHashId = (0, external_util_js_namespaceObject.midsceneGenerateHash)(node, content, rect);
        const elementInfo = {
            id: nodeHashId,
            indexId: indexId++,
            nodeHashId,
            nodeType: index_js_namespaceObject.NodeType.A,
            attributes: {
                ...attributes,
                htmlTagName: tagNameOfNode(node),
                nodeType: index_js_namespaceObject.NodeType.A
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
    if ((0, external_dom_util_js_namespaceObject.isContainerElement)(node) || isContainer) {
        const attributes = (0, external_util_js_namespaceObject.getNodeAttributes)(node, currentWindow);
        const nodeHashId = (0, external_util_js_namespaceObject.midsceneGenerateHash)(node, '', rect);
        const elementInfo = {
            id: nodeHashId,
            nodeHashId,
            indexId: indexId++,
            nodeType: index_js_namespaceObject.NodeType.CONTAINER,
            attributes: {
                ...attributes,
                nodeType: index_js_namespaceObject.NodeType.CONTAINER,
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
    return (0, external_tree_js_namespaceObject.descriptionOfTree)(elementNode, void 0, false, visibleOnly);
}
function extractTreeNode(initNode, debugMode = false) {
    (0, external_util_js_namespaceObject.setDebugMode)(debugMode);
    indexId = 0;
    const topDocument = (0, external_util_js_namespaceObject.getTopDocument)();
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
        if (elementInfo?.nodeType === index_js_namespaceObject.NodeType.BUTTON || elementInfo?.nodeType === index_js_namespaceObject.NodeType.IMG || elementInfo?.nodeType === index_js_namespaceObject.NodeType.TEXT || elementInfo?.nodeType === index_js_namespaceObject.NodeType.FORM_ITEM || elementInfo?.nodeType === index_js_namespaceObject.NodeType.CONTAINER) return nodeInfo;
        const rect = (0, external_util_js_namespaceObject.getRect)(node, baseZoom, currentWindow);
        for(let i = 0; i < node.childNodes.length; i++){
            (0, external_util_js_namespaceObject.logger)('will dfs', node.childNodes[i]);
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
    const selfRect = (0, external_util_js_namespaceObject.elementRect)(node, currentWindow, currentDocument, baseZoom);
    if (!selfRect) return null;
    let minLeft = selfRect.left;
    let minTop = selfRect.top;
    let maxRight = selfRect.left + selfRect.width;
    let maxBottom = selfRect.top + selfRect.height;
    function traverse(child) {
        for(let i = 0; i < child.childNodes.length; i++){
            const sub = child.childNodes[i];
            if (1 === sub.nodeType) {
                const rect = (0, external_util_js_namespaceObject.elementRect)(sub, currentWindow, currentDocument, baseZoom);
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
exports.collectElementInfo = __webpack_exports__.collectElementInfo;
exports.extractTextWithPosition = __webpack_exports__.extractTextWithPosition;
exports.extractTreeNode = __webpack_exports__.extractTreeNode;
exports.extractTreeNodeAsString = __webpack_exports__.extractTreeNodeAsString;
exports.mergeElementAndChildrenRects = __webpack_exports__.mergeElementAndChildrenRects;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "collectElementInfo",
    "extractTextWithPosition",
    "extractTreeNode",
    "extractTreeNodeAsString",
    "mergeElementAndChildrenRects"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});
