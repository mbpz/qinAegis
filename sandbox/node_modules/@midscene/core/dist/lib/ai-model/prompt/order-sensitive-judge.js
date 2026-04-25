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
    orderSensitiveJudgePrompt: ()=>orderSensitiveJudgePrompt,
    systemPromptToJudgeOrderSensitive: ()=>systemPromptToJudgeOrderSensitive
});
function systemPromptToJudgeOrderSensitive() {
    return `
## Role:
You are an AI assistant that analyzes UI element descriptions.

## Objective:
Determine whether a given element description is order-sensitive.

Order-sensitive descriptions contain phrases that specify position or sequence, such as:
- "the first button"
- "the second item"
- "the third row"
- "the last input"
- "the 5th element"

Order-insensitive descriptions do not specify position:
- "login button"
- "search input"
- "submit button"
- "user avatar"

## Output Format:
\`\`\`json
{
  "isOrderSensitive": boolean
}
\`\`\`

Return true if the description is order-sensitive, false otherwise.
`;
}
const orderSensitiveJudgePrompt = (description)=>`Analyze this element description: "${description}"`;
exports.orderSensitiveJudgePrompt = __webpack_exports__.orderSensitiveJudgePrompt;
exports.systemPromptToJudgeOrderSensitive = __webpack_exports__.systemPromptToJudgeOrderSensitive;
for(var __rspack_i in __webpack_exports__)if (-1 === [
    "orderSensitiveJudgePrompt",
    "systemPromptToJudgeOrderSensitive"
].indexOf(__rspack_i)) exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=order-sensitive-judge.js.map