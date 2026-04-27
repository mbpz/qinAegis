import { Agent } from "@midscene/core/agent";
import { BROWSER_NAVIGATION_ERROR_PATTERN } from "../puppeteer/base-page.mjs";
class ChromeExtensionProxyPageAgent extends Agent {
    isRetryableContextError(error) {
        return error instanceof Error && BROWSER_NAVIGATION_ERROR_PATTERN.test(error.message);
    }
}
export { ChromeExtensionProxyPageAgent };

//# sourceMappingURL=agent.mjs.map