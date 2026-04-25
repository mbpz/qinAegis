import { isSvgElement } from "./dom-util.mjs";
import { getNodeFromCacheList, getRect, isElementPartiallyInViewport, logger } from "./util.mjs";
import { collectElementInfo } from "./web-extractor.mjs";
const SUB_XPATH_SEPARATOR = '|>>|';
function parseCSSZoom(style) {
    return Number.parseFloat(style.zoom ?? '1') || 1;
}
function calculateIframeOffset(nodeOwnerDoc, rootDoc) {
    let leftOffset = 0;
    let topOffset = 0;
    let iterDoc = nodeOwnerDoc;
    while(iterDoc && iterDoc !== rootDoc)try {
        const frameElement = iterDoc.defaultView?.frameElement;
        if (!frameElement) break;
        const rect = frameElement.getBoundingClientRect();
        const parentWin = iterDoc.defaultView?.parent;
        let borderLeft = 0;
        let borderTop = 0;
        let zoom = 1;
        try {
            if (parentWin) {
                const style = parentWin.getComputedStyle(frameElement);
                borderLeft = Number.parseFloat(style.borderLeftWidth) || 0;
                borderTop = Number.parseFloat(style.borderTopWidth) || 0;
                zoom = parseCSSZoom(style);
            }
        } catch  {}
        leftOffset = leftOffset / zoom + rect.left + borderLeft;
        topOffset = topOffset / zoom + rect.top + borderTop;
        iterDoc = frameElement.ownerDocument;
    } catch  {
        break;
    }
    return {
        left: leftOffset,
        top: topOffset
    };
}
function translatePointToIframeCoordinates(point, iframeElement, parentWindow) {
    const rect = iframeElement.getBoundingClientRect();
    const style = parentWindow.getComputedStyle(iframeElement);
    const clientLeft = iframeElement.clientLeft;
    const clientTop = iframeElement.clientTop;
    const paddingLeft = Number.parseFloat(style.paddingLeft) || 0;
    const paddingTop = Number.parseFloat(style.paddingTop) || 0;
    const zoom = parseCSSZoom(style);
    return {
        left: (point.left - rect.left - clientLeft - paddingLeft) / zoom,
        top: (point.top - rect.top - clientTop - paddingTop) / zoom
    };
}
const getElementXpathIndex = (element)=>{
    let index = 1;
    let prev = element.previousElementSibling;
    while(prev){
        if (prev.nodeName.toLowerCase() === element.nodeName.toLowerCase()) index++;
        prev = prev.previousElementSibling;
    }
    return index;
};
const normalizeXpathText = (text)=>{
    if ('string' != typeof text) return '';
    return text.replace(/\s+/g, ' ').trim();
};
const buildCurrentElementXpath = (element, isOrderSensitive, isLeafElement, limitToCurrentDocument = false)=>{
    const parentPath = element.parentNode ? getElementXpath(element.parentNode, isOrderSensitive, false, limitToCurrentDocument) : '';
    const prefix = parentPath ? `${parentPath}/` : '/';
    const tagName = element.nodeName.toLowerCase();
    const textContent = element.textContent?.trim();
    const isSVGNamespace = 'http://www.w3.org/2000/svg' === element.namespaceURI;
    const tagSelector = isSVGNamespace ? `*[name()="${tagName}"]` : tagName;
    if (isOrderSensitive) {
        const index = getElementXpathIndex(element);
        return `${prefix}${tagSelector}[${index}]`;
    }
    if (isLeafElement && textContent) return `${prefix}${tagSelector}[normalize-space()="${normalizeXpathText(textContent)}"]`;
    const index = getElementXpathIndex(element);
    return `${prefix}${tagSelector}[${index}]`;
};
const getElementXpath = (element, isOrderSensitive = false, isLeafElement = false, limitToCurrentDocument = false)=>{
    if (element.nodeType === Node.TEXT_NODE) {
        const parentNode = element.parentNode;
        if (parentNode && parentNode.nodeType === Node.ELEMENT_NODE) {
            const parentXPath = getElementXpath(parentNode, isOrderSensitive, true, limitToCurrentDocument);
            const textContent = element.textContent?.trim();
            if (textContent) return `${parentXPath}/text()[normalize-space()="${normalizeXpathText(textContent)}"]`;
            return `${parentXPath}/text()`;
        }
        return '';
    }
    if (element.nodeType !== Node.ELEMENT_NODE) return '';
    const el = element;
    try {
        const nodeName = el.nodeName.toLowerCase();
        if (el === el.ownerDocument?.documentElement || 'html' === nodeName) {
            if (!limitToCurrentDocument) {
                const frameElement = el.ownerDocument?.defaultView?.frameElement;
                if (frameElement) {
                    const framePath = getElementXpath(frameElement, isOrderSensitive, false, limitToCurrentDocument);
                    return `${framePath}${SUB_XPATH_SEPARATOR}/html`;
                }
            }
            return '/html';
        }
        if (el === el.ownerDocument?.body || 'body' === nodeName) {
            if (!limitToCurrentDocument) {
                const frameElement = el.ownerDocument?.defaultView?.frameElement;
                if (frameElement) {
                    const framePath = getElementXpath(frameElement, isOrderSensitive, false, limitToCurrentDocument);
                    return `${framePath}${SUB_XPATH_SEPARATOR}/html/body`;
                }
            }
            return '/html/body';
        }
    } catch (error) {
        logger('[midscene:locator] ownerDocument access failed:', error);
        if ('html' === el.nodeName.toLowerCase()) return '/html';
        if ('body' === el.nodeName.toLowerCase()) return '/html/body';
    }
    if (isSvgElement(el)) {
        const tagName = el.nodeName.toLowerCase();
        if ('svg' === tagName) return buildCurrentElementXpath(el, isOrderSensitive, isLeafElement, limitToCurrentDocument);
        let parent = el.parentNode;
        while(parent && parent.nodeType === Node.ELEMENT_NODE){
            const parentEl = parent;
            if (!isSvgElement(parentEl)) return getElementXpath(parentEl, isOrderSensitive, isLeafElement, limitToCurrentDocument);
            const parentTag = parentEl.nodeName.toLowerCase();
            if ('svg' === parentTag) return getElementXpath(parentEl, isOrderSensitive, isLeafElement, limitToCurrentDocument);
            parent = parent.parentNode;
        }
        const fallbackParent = el.parentNode;
        if (fallbackParent && fallbackParent.nodeType === Node.ELEMENT_NODE) return getElementXpath(fallbackParent, isOrderSensitive, isLeafElement, limitToCurrentDocument);
        return '';
    }
    return buildCurrentElementXpath(el, isOrderSensitive, isLeafElement, limitToCurrentDocument);
};
function getXpathsById(id) {
    const node = getNodeFromCacheList(id);
    if (!node) return null;
    const fullXPath = getElementXpath(node, false, true, true);
    return [
        fullXPath
    ];
}
function getXpathsByPoint(point, isOrderSensitive) {
    let currentWindow = 'undefined' != typeof window ? window : void 0;
    let currentDocument = 'undefined' != typeof document ? document : void 0;
    let { left, top } = point;
    let depth = 0;
    const MAX_DEPTH = 10;
    let xpathPrefix = '';
    let lastFoundElement = null;
    while(depth < MAX_DEPTH){
        depth++;
        const element = currentDocument.elementFromPoint(left, top);
        if (!element) {
            if (lastFoundElement) {
                const fullXPath = getElementXpath(lastFoundElement, isOrderSensitive, true, true);
                return [
                    xpathPrefix + fullXPath
                ];
            }
            return null;
        }
        lastFoundElement = element;
        const tag = element.tagName.toLowerCase();
        if ('iframe' === tag || 'frame' === tag) try {
            const contentWindow = element.contentWindow;
            const contentDocument = element.contentDocument;
            if (contentWindow && contentDocument) {
                const localPoint = translatePointToIframeCoordinates({
                    left,
                    top
                }, element, currentWindow);
                const currentIframeXpath = getElementXpath(element, isOrderSensitive, false, true);
                xpathPrefix += currentIframeXpath + SUB_XPATH_SEPARATOR;
                currentWindow = contentWindow;
                currentDocument = contentDocument;
                left = localPoint.left;
                top = localPoint.top;
                continue;
            }
        } catch (error) {
            logger('[midscene:locator] iframe penetration failed (cross-origin?):', error);
        }
        const fullXPath = getElementXpath(element, isOrderSensitive, true, true);
        return [
            xpathPrefix + fullXPath
        ];
    }
    if (lastFoundElement) {
        const fullXPath = getElementXpath(lastFoundElement, isOrderSensitive, true, true);
        return [
            xpathPrefix + fullXPath
        ];
    }
    return null;
}
function getNodeInfoByXpath(xpath) {
    const parts = xpath.split(SUB_XPATH_SEPARATOR).map((p)=>p.trim()).filter(Boolean);
    if (0 === parts.length) return null;
    let currentDocument = 'undefined' != typeof document ? document : void 0;
    let node = null;
    for(let i = 0; i < parts.length; i++){
        const currentXpath = parts[i];
        const xpathResult = currentDocument.evaluate(currentXpath, currentDocument, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
        if (1 !== xpathResult.snapshotLength) {
            logger(`[midscene:locator] XPath "${currentXpath}" matched ${xpathResult.snapshotLength} elements (expected 1), discarding.`);
            return null;
        }
        node = xpathResult.snapshotItem(0);
        if (i < parts.length - 1) if (!node || node.nodeType !== Node.ELEMENT_NODE || 'iframe' !== node.tagName.toLowerCase()) return null;
        else try {
            const contentDocument = node.contentDocument;
            if (contentDocument) currentDocument = contentDocument;
            else {
                logger('[midscene:locator] iframe contentDocument is null (cross-origin?)');
                return null;
            }
        } catch (error) {
            logger('[midscene:locator] iframe contentDocument access failed:', error);
            return null;
        }
    }
    return node;
}
function getElementInfoByXpath(xpath) {
    const node = getNodeInfoByXpath(xpath);
    if (!node) return null;
    let targetWindow = 'undefined' != typeof window ? window : void 0;
    let targetDocument = 'undefined' != typeof document ? document : void 0;
    if (node.ownerDocument?.defaultView) {
        targetWindow = node.ownerDocument.defaultView;
        targetDocument = node.ownerDocument;
    }
    const rootDoc = 'undefined' != typeof document ? document : null;
    const iframeOffset = calculateIframeOffset(node.ownerDocument ?? null, rootDoc);
    const targetWin = targetWindow;
    const targetDoc = targetDocument;
    if (node instanceof targetWin.HTMLElement) {
        const rect = getRect(node, 1, targetWin);
        const isVisible = isElementPartiallyInViewport(rect, targetWin, targetDoc, 1);
        if (!isVisible) node.scrollIntoView({
            behavior: 'instant',
            block: 'center'
        });
    }
    return collectElementInfo(node, targetWin, targetDoc, 1, iframeOffset, true);
}
export { getElementInfoByXpath, getElementXpath, getNodeInfoByXpath, getXpathsById, getXpathsByPoint };
