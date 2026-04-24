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
    setDebugMode: ()=>setDebugMode,
    getNodeAttributes: ()=>getNodeAttributes,
    getNodeFromCacheList: ()=>getNodeFromCacheList,
    hasOverflowY: ()=>hasOverflowY,
    setExtractTextWithPositionOnWindow: ()=>setExtractTextWithPositionOnWindow,
    setGenerateHashOnWindow: ()=>setGenerateHashOnWindow,
    getTopDocument: ()=>getTopDocument,
    overlappedRect: ()=>overlappedRect,
    isElementPartiallyInViewport: ()=>isElementPartiallyInViewport,
    getPseudoElementContent: ()=>getPseudoElementContent,
    midsceneGenerateHash: ()=>midsceneGenerateHash,
    getDebugMode: ()=>getDebugMode,
    logger: ()=>logger,
    elementRect: ()=>elementRect,
    setMidsceneVisibleRectOnWindow: ()=>setMidsceneVisibleRectOnWindow,
    getRect: ()=>getRect,
    generateId: ()=>generateId,
    setNodeToCacheList: ()=>setNodeToCacheList,
    validTextNodeContent: ()=>validTextNodeContent,
    setNodeHashCacheListOnWindow: ()=>setNodeHashCacheListOnWindow
});
const external_utils_js_namespaceObject = require("../utils.js");
const external_web_extractor_js_namespaceObject = require("./web-extractor.js");
const MAX_VALUE_LENGTH = 300;
let debugMode = false;
function setDebugMode(mode) {
    debugMode = mode;
}
function getDebugMode() {
    return debugMode;
}
function logger(..._msg) {
    if (!debugMode) return;
    console.log(..._msg);
}
function isElementPartiallyInViewport(rect, currentWindow, currentDocument, visibleAreaRatio = 2 / 3) {
    const elementHeight = rect.height;
    const elementWidth = rect.width;
    const viewportRect = {
        left: 0,
        top: 0,
        width: currentWindow.innerWidth || currentDocument.documentElement.clientWidth,
        height: currentWindow.innerHeight || currentDocument.documentElement.clientHeight,
        right: currentWindow.innerWidth || currentDocument.documentElement.clientWidth,
        bottom: currentWindow.innerHeight || currentDocument.documentElement.clientHeight,
        x: 0,
        y: 0,
        zoom: 1
    };
    const overlapRect = overlappedRect(rect, viewportRect);
    if (!overlapRect) return false;
    const visibleArea = overlapRect.width * overlapRect.height;
    const totalArea = elementHeight * elementWidth;
    return visibleArea / totalArea >= visibleAreaRatio;
}
function getPseudoElementContent(element, currentWindow) {
    if (!(element instanceof currentWindow.HTMLElement)) return {
        before: '',
        after: ''
    };
    const beforeContent = currentWindow.getComputedStyle(element, '::before').getPropertyValue('content');
    const afterContent = currentWindow.getComputedStyle(element, '::after').getPropertyValue('content');
    return {
        before: 'none' === beforeContent ? '' : beforeContent.replace(/"/g, ''),
        after: 'none' === afterContent ? '' : afterContent.replace(/"/g, '')
    };
}
function hasOverflowY(element, currentWindow) {
    const style = currentWindow.getComputedStyle(element);
    return 'scroll' === style.overflowY || 'auto' === style.overflowY || 'hidden' === style.overflowY;
}
function overlappedRect(rect1, rect2) {
    const left = Math.max(rect1.left, rect2.left);
    const top = Math.max(rect1.top, rect2.top);
    const right = Math.min(rect1.right, rect2.right);
    const bottom = Math.min(rect1.bottom, rect2.bottom);
    if (left < right && top < bottom) return {
        left,
        top,
        right,
        bottom,
        width: right - left,
        height: bottom - top,
        x: left,
        y: top,
        zoom: 1
    };
    return null;
}
function getRect(el, baseZoom, currentWindow) {
    let originalRect;
    let newZoom = 1;
    const hasGetBoundingClientRect = el instanceof Element;
    if (hasGetBoundingClientRect) {
        originalRect = el.getBoundingClientRect();
        if (el instanceof currentWindow.HTMLElement && !('currentCSSZoom' in el)) newZoom = Number.parseFloat(currentWindow.getComputedStyle(el).zoom) || 1;
    } else {
        const range = currentWindow.document.createRange();
        range.selectNodeContents(el);
        originalRect = range.getBoundingClientRect();
    }
    const zoom = newZoom * baseZoom;
    return {
        width: originalRect.width * zoom,
        height: originalRect.height * zoom,
        left: originalRect.left * zoom,
        top: originalRect.top * zoom,
        right: originalRect.right * zoom,
        bottom: originalRect.bottom * zoom,
        x: originalRect.x * zoom,
        y: originalRect.y * zoom,
        zoom
    };
}
const isElementCovered = (el, rect, currentWindow)=>{
    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;
    const topElement = currentWindow.document.elementFromPoint(x, y);
    if (!topElement) return false;
    if (topElement === el) return false;
    if (el?.contains(topElement)) return false;
    if (topElement?.contains(el)) return false;
    const rectOfTopElement = getRect(topElement, 1, currentWindow);
    const overlapRect = overlappedRect(rect, rectOfTopElement);
    if (!overlapRect) return false;
    logger(el, 'Element is covered by another element', {
        topElement,
        el,
        rect,
        x,
        y
    });
    return true;
};
function elementRect(el, currentWindow, currentDocument, baseZoom = 1) {
    if (!el) {
        logger(el, 'Element is not in the DOM hierarchy');
        return false;
    }
    if (!(el instanceof currentWindow.HTMLElement) && el.nodeType !== Node.TEXT_NODE && 'svg' !== el.nodeName.toLowerCase()) {
        logger(el, 'Element is not in the DOM hierarchy');
        return false;
    }
    if (el instanceof currentWindow.HTMLElement) {
        const style = currentWindow.getComputedStyle(el);
        if ('none' === style.display || 'hidden' === style.visibility || '0' === style.opacity && 'INPUT' !== el.tagName) {
            logger(el, 'Element is hidden');
            return false;
        }
    }
    const rect = getRect(el, baseZoom, currentWindow);
    if (0 === rect.width && 0 === rect.height) {
        logger(el, 'Element has no size');
        return false;
    }
    if (1 === baseZoom && isElementCovered(el, rect, currentWindow)) return false;
    const isVisible = isElementPartiallyInViewport(rect, currentWindow, currentDocument);
    let parent = el;
    const parentUntilNonStatic = (currentNode)=>{
        let parent = currentNode?.parentElement;
        while(parent){
            const style = currentWindow.getComputedStyle(parent);
            if ('static' !== style.position) return parent;
            parent = parent.parentElement;
        }
        return null;
    };
    while(parent && parent !== currentDocument.body){
        if (!(parent instanceof currentWindow.HTMLElement)) {
            parent = parent.parentElement;
            continue;
        }
        const parentStyle = currentWindow.getComputedStyle(parent);
        if ('hidden' === parentStyle.overflow) {
            const parentRect = getRect(parent, 1, currentWindow);
            const tolerance = 10;
            if (rect.right < parentRect.left - tolerance || rect.left > parentRect.right + tolerance || rect.bottom < parentRect.top - tolerance || rect.top > parentRect.bottom + tolerance) {
                logger(el, 'element is partially or totally hidden by an ancestor', {
                    rect,
                    parentRect
                });
                return false;
            }
        }
        if ('fixed' === parentStyle.position || 'sticky' === parentStyle.position) break;
        parent = 'absolute' === parentStyle.position ? parentUntilNonStatic(parent) : parent.parentElement;
    }
    return {
        left: Math.round(rect.left),
        top: Math.round(rect.top),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
        zoom: rect.zoom,
        isVisible
    };
}
function validTextNodeContent(node) {
    if (!node) return false;
    if (node.nodeType !== Node.ELEMENT_NODE && node.nodeType !== Node.TEXT_NODE && '#text' !== node.nodeName) return false;
    const content = node.textContent || node.innerText;
    if (content && !/^\s*$/.test(content)) return content.trim();
    return false;
}
function getNodeAttributes(node, currentWindow) {
    if (!node || !(node instanceof currentWindow.HTMLElement) || !node.attributes) return {};
    const attributesList = Array.from(node.attributes).map((attr)=>{
        if ('class' === attr.name) return [
            attr.name,
            `.${attr.value.split(' ').join('.')}`
        ];
        let value = attr.value;
        if (value.startsWith('data:image')) value = 'image';
        if (value.length > MAX_VALUE_LENGTH) value = `${value.slice(0, MAX_VALUE_LENGTH)}...`;
        return [
            attr.name,
            value
        ];
    });
    return Object.fromEntries(attributesList);
}
const NODE_CACHE_MAX_SIZE = 2000;
function setNodeHashCacheListOnWindow() {
    if ('undefined' != typeof window) window.midsceneNodeHashCache = new Map();
}
function getNodeCacheMap() {
    if ('undefined' == typeof window) return;
    return window.midsceneNodeHashCache;
}
function setNodeToCacheList(node, id) {
    const cache = getNodeCacheMap();
    if (!cache) return;
    if (cache.has(id)) return;
    if (cache.size >= NODE_CACHE_MAX_SIZE) {
        const firstKey = cache.keys().next().value;
        if (void 0 !== firstKey) cache.delete(firstKey);
    }
    cache.set(id, node);
}
function getNodeFromCacheList(id) {
    return getNodeCacheMap()?.get(id);
}
function midsceneGenerateHash(node, content, rect) {
    const slicedHash = (0, external_utils_js_namespaceObject.generateHashId)(rect, content);
    if (node) {
        if ('undefined' != typeof window && !getNodeCacheMap()) setNodeHashCacheListOnWindow();
        setNodeToCacheList(node, slicedHash);
    }
    return slicedHash;
}
function generateId(numberId) {
    return `${numberId}`;
}
function setGenerateHashOnWindow() {
    if ('undefined' != typeof window) window.midsceneGenerateHash = midsceneGenerateHash;
}
function setMidsceneVisibleRectOnWindow() {
    if ('undefined' != typeof window) window.midsceneVisibleRect = elementRect;
}
function setExtractTextWithPositionOnWindow() {
    if ('undefined' != typeof window) window.extractTextWithPosition = external_web_extractor_js_namespaceObject.extractTextWithPosition;
}
function getTopDocument() {
    const container = document.body || document;
    return container;
}
exports.elementRect = __webpack_exports__.elementRect;
exports.generateId = __webpack_exports__.generateId;
exports.getDebugMode = __webpack_exports__.getDebugMode;
exports.getNodeAttributes = __webpack_exports__.getNodeAttributes;
exports.getNodeFromCacheList = __webpack_exports__.getNodeFromCacheList;
exports.getPseudoElementContent = __webpack_exports__.getPseudoElementContent;
exports.getRect = __webpack_exports__.getRect;
exports.getTopDocument = __webpack_exports__.getTopDocument;
exports.hasOverflowY = __webpack_exports__.hasOverflowY;
exports.isElementPartiallyInViewport = __webpack_exports__.isElementPartiallyInViewport;
exports.logger = __webpack_exports__.logger;
exports.midsceneGenerateHash = __webpack_exports__.midsceneGenerateHash;
exports.overlappedRect = __webpack_exports__.overlappedRect;
exports.setDebugMode = __webpack_exports__.setDebugMode;
exports.setExtractTextWithPositionOnWindow = __webpack_exports__.setExtractTextWithPositionOnWindow;
exports.setGenerateHashOnWindow = __webpack_exports__.setGenerateHashOnWindow;
exports.setMidsceneVisibleRectOnWindow = __webpack_exports__.setMidsceneVisibleRectOnWindow;
exports.setNodeHashCacheListOnWindow = __webpack_exports__.setNodeHashCacheListOnWindow;
exports.setNodeToCacheList = __webpack_exports__.setNodeToCacheList;
exports.validTextNodeContent = __webpack_exports__.validTextNodeContent;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "elementRect",
    "generateId",
    "getDebugMode",
    "getNodeAttributes",
    "getNodeFromCacheList",
    "getPseudoElementContent",
    "getRect",
    "getTopDocument",
    "hasOverflowY",
    "isElementPartiallyInViewport",
    "logger",
    "midsceneGenerateHash",
    "overlappedRect",
    "setDebugMode",
    "setExtractTextWithPositionOnWindow",
    "setGenerateHashOnWindow",
    "setMidsceneVisibleRectOnWindow",
    "setNodeHashCacheListOnWindow",
    "setNodeToCacheList",
    "validTextNodeContent"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});
