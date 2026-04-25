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
    generateElementByPoint: ()=>generateElementByPoint,
    generateElementByRect: ()=>generateElementByRect,
    isAElement: ()=>isAElement,
    isButtonElement: ()=>isButtonElement,
    isContainerElement: ()=>isContainerElement,
    isFormElement: ()=>isFormElement,
    isImgElement: ()=>isImgElement,
    isNotContainerElement: ()=>isNotContainerElement,
    isSvgElement: ()=>isSvgElement,
    isTextElement: ()=>isTextElement
});
function isFormElement(node) {
    return node instanceof HTMLElement && ('input' === node.tagName.toLowerCase() || 'textarea' === node.tagName.toLowerCase() || 'select' === node.tagName.toLowerCase() || 'option' === node.tagName.toLowerCase());
}
function isButtonElement(node) {
    return node instanceof HTMLElement && 'button' === node.tagName.toLowerCase();
}
function isAElement(node) {
    return node instanceof HTMLElement && 'a' === node.tagName.toLowerCase();
}
function isSvgElement(node) {
    return node instanceof SVGElement;
}
function isImgElement(node) {
    if (!includeBaseElement(node) && node instanceof Element) {
        const computedStyle = window.getComputedStyle(node);
        const backgroundImage = computedStyle.getPropertyValue('background-image');
        if ('none' !== backgroundImage) return true;
    }
    if (isIconfont(node)) return true;
    return node instanceof HTMLElement && 'img' === node.tagName.toLowerCase() || node instanceof SVGElement && 'svg' === node.tagName.toLowerCase();
}
function isIconfont(node) {
    if (node instanceof Element) {
        const computedStyle = window.getComputedStyle(node);
        const fontFamilyValue = computedStyle.fontFamily || '';
        return fontFamilyValue.toLowerCase().indexOf('iconfont') >= 0;
    }
    return false;
}
function isNotContainerElement(node) {
    return isTextElement(node) || isIconfont(node) || isImgElement(node) || isButtonElement(node) || isAElement(node) || isFormElement(node);
}
function isTextElement(node) {
    if (node instanceof Element) {
        if (node?.childNodes?.length === 1 && node?.childNodes[0] instanceof Text) return true;
    }
    return node.nodeName?.toLowerCase?.() === '#text' && !isIconfont(node);
}
function isContainerElement(node) {
    if (!(node instanceof HTMLElement)) return false;
    if (includeBaseElement(node)) return false;
    const computedStyle = window.getComputedStyle(node);
    const backgroundColor = computedStyle.getPropertyValue('background-color');
    if (backgroundColor) return true;
    return false;
}
function includeBaseElement(node) {
    if (!(node instanceof HTMLElement)) return false;
    if (node.innerText) return true;
    const includeList = [
        'svg',
        'button',
        'input',
        'textarea',
        'select',
        'option',
        'img',
        'a'
    ];
    for (const tagName of includeList){
        const element = node.querySelectorAll(tagName);
        if (element.length > 0) return true;
    }
    return false;
}
function generateElementByPoint(center, description, edgeSize = 8) {
    const [centerX, centerY] = center;
    const offset = Math.ceil(edgeSize / 2) - 1;
    const expandedRect = {
        left: Math.max(centerX - offset, 0),
        top: Math.max(centerY - offset, 0),
        width: edgeSize,
        height: edgeSize
    };
    return {
        rect: expandedRect,
        center: [
            centerX,
            centerY
        ],
        description: description || ''
    };
}
function generateElementByRect(sourceRect, description, _edgeSize = 8) {
    const centerX = sourceRect.left + Math.floor((sourceRect.width - 1) / 2);
    const centerY = sourceRect.top + Math.floor((sourceRect.height - 1) / 2);
    return {
        rect: sourceRect,
        center: [
            centerX,
            centerY
        ],
        description: description || ''
    };
}
exports.generateElementByPoint = __webpack_exports__.generateElementByPoint;
exports.generateElementByRect = __webpack_exports__.generateElementByRect;
exports.isAElement = __webpack_exports__.isAElement;
exports.isButtonElement = __webpack_exports__.isButtonElement;
exports.isContainerElement = __webpack_exports__.isContainerElement;
exports.isFormElement = __webpack_exports__.isFormElement;
exports.isImgElement = __webpack_exports__.isImgElement;
exports.isNotContainerElement = __webpack_exports__.isNotContainerElement;
exports.isSvgElement = __webpack_exports__.isSvgElement;
exports.isTextElement = __webpack_exports__.isTextElement;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "generateElementByPoint",
    "generateElementByRect",
    "isAElement",
    "isButtonElement",
    "isContainerElement",
    "isFormElement",
    "isImgElement",
    "isNotContainerElement",
    "isSvgElement",
    "isTextElement"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});
