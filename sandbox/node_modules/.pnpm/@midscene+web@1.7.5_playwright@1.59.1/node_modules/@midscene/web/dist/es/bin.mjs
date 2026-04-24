import { launchPreparedPlaygroundPlatform } from "@midscene/playground";
import "dotenv/config";
import { webPlaygroundPlatform } from "./platform.mjs";
async function startServer() {
    const prepared = await webPlaygroundPlatform.prepare({
        launchOptions: {
            openBrowser: false,
            verbose: false
        }
    });
    const { server } = await launchPreparedPlaygroundPlatform(prepared);
    console.log(`Midscene playground server is running on http://localhost:${server.port}`);
}
startServer().catch(console.error);

//# sourceMappingURL=bin.mjs.map