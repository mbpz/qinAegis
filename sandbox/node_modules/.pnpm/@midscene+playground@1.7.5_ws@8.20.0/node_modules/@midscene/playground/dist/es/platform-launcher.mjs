import { playgroundForAgent, playgroundForAgentFactory, playgroundForSessionManager } from "./launcher.mjs";
import { resolvePreparedLaunchOptions } from "./platform.mjs";
async function launchPreparedPlaygroundPlatform(prepared, overrides = {}) {
    const launchOptions = resolvePreparedLaunchOptions(prepared, overrides);
    const applyPreparedPlatform = (result)=>{
        result.server.setPreparedPlatform(prepared);
        return result;
    };
    const startPreparedSidecars = async ()=>{
        if (prepared.sessionManager) return;
        for (const sidecar of prepared.sidecars || [])await sidecar.start();
    };
    if (prepared.agentFactory) {
        await startPreparedSidecars();
        return applyPreparedPlatform(await playgroundForAgentFactory(prepared.agentFactory).launch(launchOptions));
    }
    if (prepared.agent) {
        await startPreparedSidecars();
        return applyPreparedPlatform(await playgroundForAgent(prepared.agent).launch(launchOptions));
    }
    if (prepared.sessionManager) return applyPreparedPlatform(await playgroundForSessionManager().launch(launchOptions));
    throw new Error(`Prepared platform "${prepared.platformId}" must provide agent, agentFactory, or sessionManager`);
}
export { launchPreparedPlaygroundPlatform };

//# sourceMappingURL=platform-launcher.mjs.map