"use strict";
var __webpack_exports__ = {};
const playground_namespaceObject = require("@midscene/playground");
require("dotenv/config");
const external_platform_js_namespaceObject = require("./platform.js");
async function startServer() {
    const prepared = await external_platform_js_namespaceObject.webPlaygroundPlatform.prepare({
        launchOptions: {
            openBrowser: false,
            verbose: false
        }
    });
    const { server } = await (0, playground_namespaceObject.launchPreparedPlaygroundPlatform)(prepared);
    console.log(`Midscene playground server is running on http://localhost:${server.port}`);
}
startServer().catch(console.error);
for(var __rspack_i in __webpack_exports__)exports[__rspack_i] = __webpack_exports__[__rspack_i];
Object.defineProperty(exports, '__esModule', {
    value: true
});

//# sourceMappingURL=bin.js.map